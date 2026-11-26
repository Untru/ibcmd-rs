//! The main search-information row of `Params` (`<uuid>.si`).
//!
//! A native apply rewrites every `<uuid>.si` row and gives each a new version
//! in `siVersions`. The contents only change with the set of metadata objects
//! and with the synonyms of the ones that stay (an object whose synonym is
//! changed gets the new text in its record: `set_synonyms`); the main row lists
//! them all -- every owner, then the forms, templates, attributes and commands
//! that belong to it -- and gains one record for each new form or template:
//!
//! ```text
//! {4,
//! {106,<class id>,<class id>,...},
//! {10809,<uuid>,<parent uuid>,<kind>,"<name>",
//! {1,1,
//! {"ru","<synonym>"}
//! },<flag>,<flag>,<uuid>,...}
//! }
//! ```
//!
//! A record is seven members; its *kind* is the index of the object's class id
//! in the first list (a form of a catalog and a form of a data processor are
//! different classes; every template is `3daea016-...`). Records come in
//! depth-first order, and an owner's children in a fixed order of their
//! classes (measured: attributes, tabular sections, forms, templates,
//! commands for a data processor). This module edits the text in place, so
//! that everything it does not touch stays byte for byte as it was.

use std::collections::{BTreeSet, HashMap};

use anyhow::{Result, anyhow, bail, ensure};
use uuid::Uuid;

/// One record of the object list, with the byte span it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiRecord {
    pub start: usize,
    pub end: usize,
    pub uuid: String,
    pub parent: String,
    pub kind: usize,
    pub name: String,
    /// The span of the synonym block (`{1,<n>,{"ru","text"},...}`).
    pub synonyms: (usize, usize),
}

/// The parsed main row.
#[derive(Debug)]
pub struct SiMain {
    /// The class ids, lower-cased; a record's kind indexes this list.
    pub classes: Vec<String>,
    pub records: Vec<SiRecord>,
    /// The span of the record count.
    count_span: (usize, usize),
    by_uuid: HashMap<String, usize>,
    /// For every record, the index one past its last descendant.
    ends: Vec<usize>,
    /// The line break the row is written with (the platform writes CRLF).
    pub eol: &'static str,
}

struct Scanner<'a> {
    text: &'a [u8],
    pos: usize,
}

fn skip_string(text: &[u8], mut pos: usize) -> Result<usize> {
    let open = pos;
    pos += 1;
    loop {
        match text.get(pos) {
            None => bail!("unclosed string at byte {open}"),
            Some(b'"') if text.get(pos + 1) == Some(&b'"') => pos += 2,
            Some(b'"') => return Ok(pos + 1),
            Some(_) => pos += 1,
        }
    }
}

impl<'a> Scanner<'a> {
    fn new(text: &'a [u8]) -> Self {
        let pos = if text.starts_with(&[0xef, 0xbb, 0xbf]) {
            3
        } else {
            0
        };
        Self { text, pos }
    }

    fn skip_ws(&mut self) {
        while matches!(self.text.get(self.pos), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<()> {
        self.skip_ws();
        if self.text.get(self.pos) == Some(&byte) {
            self.pos += 1;
            Ok(())
        } else {
            bail!("expected {:?} at byte {}", byte as char, self.pos)
        }
    }

    /// One member (an atom, a string or a whole list): its byte span.
    fn member(&mut self) -> Result<(usize, usize)> {
        self.skip_ws();
        let start = self.pos;
        match self.text.get(self.pos) {
            None => bail!("unexpected end of the text at byte {start}"),
            Some(b'"') => self.pos = skip_string(self.text, self.pos)?,
            Some(b'{') => {
                let mut depth = 0usize;
                loop {
                    match self.text.get(self.pos) {
                        None => bail!("unclosed list opened at byte {start}"),
                        Some(b'"') => self.pos = skip_string(self.text, self.pos)?,
                        Some(b'{') => {
                            depth += 1;
                            self.pos += 1;
                        }
                        Some(b'}') => {
                            depth -= 1;
                            self.pos += 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Some(_) => self.pos += 1,
                    }
                }
            }
            Some(_) => {
                while let Some(byte) = self.text.get(self.pos) {
                    if matches!(byte, b',' | b'}' | b' ' | b'\t' | b'\r' | b'\n') {
                        break;
                    }
                    self.pos += 1;
                }
                ensure!(self.pos > start, "an empty member at byte {start}");
            }
        }
        Ok((start, self.pos))
    }

    /// After a member: `true` when a comma follows, `false` at the list's
    /// closing brace (consumed).
    fn separator(&mut self) -> Result<bool> {
        self.skip_ws();
        match self.text.get(self.pos) {
            Some(b',') => {
                self.pos += 1;
                Ok(true)
            }
            Some(b'}') => {
                self.pos += 1;
                Ok(false)
            }
            _ => bail!("expected ',' or '}}' at byte {}", self.pos),
        }
    }

    /// The members of the list that starts here.
    fn list(&mut self) -> Result<Vec<(usize, usize)>> {
        self.expect(b'{')?;
        let mut members = vec![self.member()?];
        while self.separator()? {
            members.push(self.member()?);
        }
        Ok(members)
    }
}

fn atom(text: &[u8], span: (usize, usize)) -> Result<&str> {
    std::str::from_utf8(&text[span.0..span.1]).map_err(|_| anyhow!("a member is not UTF-8"))
}

fn unquote(text: &[u8], span: (usize, usize)) -> Result<String> {
    let raw = atom(text, span)?;
    let inner = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .ok_or_else(|| anyhow!("a name is not a string: {raw}"))?;
    Ok(inner.replace("\"\"", "\""))
}

/// Parses the row's text (BOM optional).
pub fn parse(text: &[u8]) -> Result<SiMain> {
    let mut scanner = Scanner::new(text);
    scanner.expect(b'{')?;
    let version = scanner.member()?;
    ensure!(
        atom(text, version)? == "4",
        "not a search-information row: it starts with {}",
        atom(text, version)?
    );
    ensure!(scanner.separator()?, "the row has no class list");
    let class_members = scanner.list()?;
    let declared = atom(text, class_members[0])?
        .parse::<usize>()
        .map_err(|_| anyhow!("the class list has no count"))?;
    ensure!(
        class_members.len() == declared + 1,
        "the class list declares {declared} ids and holds {}",
        class_members.len() - 1
    );
    let classes = class_members[1..]
        .iter()
        .map(|span| atom(text, *span).map(str::to_ascii_lowercase))
        .collect::<Result<Vec<_>>>()?;
    ensure!(scanner.separator()?, "the row has no record list");
    let members = scanner.list()?;
    ensure!(
        !scanner.separator()?,
        "the row has members after its record list"
    );
    scanner.skip_ws();
    ensure!(
        scanner.pos == text.len(),
        "text after the row at byte {}",
        scanner.pos
    );
    let count = atom(text, members[0])?
        .parse::<usize>()
        .map_err(|_| anyhow!("the record list has no count"))?;
    ensure!(
        members.len() == 1 + 7 * count,
        "the record list declares {count} records and holds {} members",
        members.len() - 1
    );
    let mut records = Vec::with_capacity(count);
    let mut by_uuid = HashMap::with_capacity(count);
    for group in members[1..].chunks(7) {
        let uuid = atom(text, group[0])?.to_ascii_lowercase();
        let parent = atom(text, group[1])?.to_ascii_lowercase();
        ensure!(
            crate::mssql_config_apply::model::is_uuid_text(&uuid)
                && crate::mssql_config_apply::model::is_uuid_text(&parent),
            "a record does not start with two ids at byte {}",
            group[0].0
        );
        let kind = atom(text, group[2])?
            .parse::<usize>()
            .map_err(|_| anyhow!("a record's kind is not a number at byte {}", group[2].0))?;
        ensure!(
            text[group[3].0] == b'"' && text[group[4].0] == b'{',
            "a record's name or synonyms are misplaced at byte {}",
            group[3].0
        );
        by_uuid.entry(uuid.clone()).or_insert(records.len());
        records.push(SiRecord {
            start: group[0].0,
            end: group[6].1,
            uuid,
            parent,
            kind,
            name: unquote(text, group[3])?,
            synonyms: group[4],
        });
    }
    // Depth-first order: a record's descendants follow it.
    let mut depth = vec![0usize; records.len()];
    for (index, record) in records.iter().enumerate() {
        depth[index] = match by_uuid.get(&record.parent) {
            Some(parent) if *parent < index => depth[*parent] + 1,
            _ => 0,
        };
    }
    let mut ends = vec![records.len(); records.len()];
    let mut open: Vec<usize> = Vec::new();
    for index in 0..records.len() {
        while let Some(&top) = open.last() {
            if depth[top] >= depth[index] {
                ends[top] = index;
                open.pop();
            } else {
                break;
            }
        }
        open.push(index);
    }
    let eol = if text.windows(2).any(|pair| pair == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    Ok(SiMain {
        classes,
        records,
        count_span: members[0],
        by_uuid,
        ends,
        eol,
    })
}

impl SiMain {
    pub fn index_of(&self, uuid: &str) -> Option<usize> {
        self.by_uuid.get(&uuid.to_ascii_lowercase()).copied()
    }

    /// The index one past the last descendant of record `index`.
    pub fn subtree_end(&self, index: usize) -> usize {
        self.ends[index]
    }

    /// The direct children of a record, in list order.
    pub fn children(&self, index: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut next = index + 1;
        while next < self.subtree_end(index) {
            out.push(next);
            next = self.subtree_end(next);
        }
        out
    }

    /// The index of a class id in the class list -- the kind a record of that
    /// class carries.
    pub fn kind_of_class(&self, class: &str) -> Option<usize> {
        let class = class.to_ascii_lowercase();
        self.classes.iter().position(|known| *known == class)
    }

    /// The children of `owner`'s kind, kind by kind, as every owner of that
    /// kind lists them: `(a, b)` when some owner lists a group of kind `a`
    /// before one of kind `b`, closed under transitivity.
    pub fn child_order(&self, owner_kind: usize) -> Result<BTreeSet<(usize, usize)>> {
        let mut pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
        for index in 0..self.records.len() {
            if self.records[index].kind != owner_kind {
                continue;
            }
            let mut sequence: Vec<usize> = Vec::new();
            for child in self.children(index) {
                let kind = self.records[child].kind;
                if sequence.last() != Some(&kind) {
                    ensure!(
                        !sequence.contains(&kind),
                        "the children of {} are not grouped by kind",
                        self.records[index].name
                    );
                    sequence.push(kind);
                }
            }
            for (position, before) in sequence.iter().enumerate() {
                for after in &sequence[position + 1..] {
                    pairs.insert((*before, *after));
                }
            }
        }
        loop {
            let mut added = Vec::new();
            for (a, b) in &pairs {
                for (c, d) in &pairs {
                    if b == c && !pairs.contains(&(*a, *d)) {
                        added.push((*a, *d));
                    }
                }
            }
            if added.is_empty() {
                break;
            }
            pairs.extend(added);
        }
        for (a, b) in &pairs {
            ensure!(
                !pairs.contains(&(*b, *a)),
                "owners of kind {owner_kind} list the kinds {a} and {b} in both orders"
            );
        }
        Ok(pairs)
    }

    /// Where a new child of `owner` with class kind `kind` goes: the index of
    /// the record to insert before (`records.len()` to append). `previous` and
    /// `next` are the nearest listed siblings of the same group that the row
    /// already holds.
    pub fn place(
        &self,
        owner: &str,
        kind: usize,
        previous: Option<&str>,
        next: Option<&str>,
    ) -> Result<usize> {
        let owner_index = self
            .index_of(owner)
            .ok_or_else(|| anyhow!("the search information does not list the owner {owner}"))?;
        let sibling = |uuid: &str| -> Result<usize> {
            let index = self
                .index_of(uuid)
                .ok_or_else(|| anyhow!("the search information does not list {uuid}"))?;
            ensure!(
                self.records[index].parent == self.records[owner_index].uuid
                    && self.records[index].kind == kind,
                "{uuid} is not a child of kind {kind} of the owner"
            );
            Ok(index)
        };
        if let Some(previous) = previous {
            return Ok(self.subtree_end(sibling(previous)?));
        }
        if let Some(next) = next {
            return sibling(next);
        }
        // The first of its group: between the groups that come before and the
        // groups that come after, in the order the other owners of this kind
        // list them.
        let children = self.children(owner_index);
        let end = self.subtree_end(owner_index);
        if children.is_empty() {
            return Ok(end);
        }
        ensure!(
            children
                .iter()
                .all(|child| self.records[*child].kind != kind),
            "the owner already lists children of kind {kind} that are no anchor"
        );
        let order = self.child_order(self.records[owner_index].kind)?;
        let mut insert_at = None;
        let mut passed_after = false;
        for child in &children {
            let other = self.records[*child].kind;
            if order.contains(&(other, kind)) {
                ensure!(
                    !passed_after,
                    "the children of {} are not in the usual order",
                    self.records[owner_index].name
                );
                insert_at = Some(self.subtree_end(*child));
            } else if order.contains(&(kind, other)) {
                if !passed_after {
                    passed_after = true;
                    if insert_at.is_none() {
                        insert_at = Some(*child);
                    }
                }
            } else {
                bail!(
                    "no owner of kind {} lists children of kind {kind} and {other} together: their order is unknown",
                    self.records[owner_index].kind
                );
            }
        }
        insert_at.ok_or_else(|| anyhow!("cannot place a child of kind {kind}"))
    }
}

/// The synonym block of a record as the platform writes it: `{1,0}` for none, otherwise `{1,<n>,` and the
/// `{"language","text"}` pairs sorted by language, one to a line, and `}`. The texts are as they stand between
/// the quotes (quotes doubled).
fn synonym_block(synonyms: &[(String, String)], eol: &str) -> String {
    let mut synonyms = synonyms.to_vec();
    synonyms.sort();
    if synonyms.is_empty() {
        return "{1,0}".to_owned();
    }
    let pairs = synonyms
        .iter()
        .map(|(language, text)| format!("{{\"{language}\",\"{text}\"}}"))
        .collect::<Vec<_>>()
        .join(&format!(",{eol}"));
    format!("{{1,{},{eol}{pairs}{eol}}}", synonyms.len())
}

/// The text of one new record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRecord {
    pub uuid: String,
    pub parent: String,
    pub kind: usize,
    pub name: String,
    /// `(language, text)` with the text as it stands between the quotes of the
    /// descriptor (quotes doubled).
    pub synonyms: Vec<(String, String)>,
    pub flags: (u8, u8),
}

impl NewRecord {
    /// The record as the platform writes it, with the row's line break.
    pub fn render(&self, eol: &str) -> String {
        let block = synonym_block(&self.synonyms, eol);
        format!(
            "{},{},{},\"{}\",{eol}{block},{},{}",
            self.uuid, self.parent, self.kind, self.name, self.flags.0, self.flags.1
        )
    }
}

/// Records to put in before the record at `at` (the list's length: at the end).
#[derive(Debug, Clone)]
pub struct Insertion {
    pub at: usize,
    pub records: Vec<NewRecord>,
}

/// The row's text with the records inserted and the count raised; nothing
/// else changes.
pub fn insert_records(text: &[u8], main: &SiMain, insertions: &[Insertion]) -> Result<Vec<u8>> {
    edit_records(text, main, insertions, &[])
}

/// The row's text with the synonym block of the records `changes` names (by their index in
/// [`SiMain::records`], the texts as they stand between the quotes, quotes doubled) written anew; a record
/// whose block is already what would be written stays as it is, and nothing else changes. Returns the text
/// and how many records were rewritten.
pub fn set_synonyms(
    text: &[u8],
    main: &SiMain,
    changes: &[(usize, Vec<(String, String)>)],
) -> Result<(Vec<u8>, usize)> {
    let mut edits: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    for (index, synonyms) in changes {
        let record = main
            .records
            .get(*index)
            .ok_or_else(|| anyhow!("a synonym change beyond the record list"))?;
        let block = synonym_block(synonyms, main.eol).into_bytes();
        if text[record.synonyms.0..record.synonyms.1] != block[..] {
            edits.push((record.synonyms.0, record.synonyms.1, block));
        }
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = Vec::with_capacity(text.len() + 64 * edits.len());
    let mut copied = 0usize;
    let count = edits.len();
    for (start, end, replacement) in edits {
        ensure!(start >= copied, "overlapping synonym edits");
        out.extend_from_slice(&text[copied..start]);
        out.extend_from_slice(&replacement);
        copied = end;
    }
    out.extend_from_slice(&text[copied..]);
    Ok((out, count))
}

/// The runs of consecutive removed items of a list, each as the span of text that goes. An
/// item goes with the comma that follows it (the comma in front of it when the run closes the
/// list), so what stays is what the platform writes for the smaller list. `spans` are the
/// `(start, end)` of every item in list order, `removed` their indices, sorted and unique.
fn cuts(spans: &[(usize, usize)], removed: &[usize]) -> Result<Vec<(usize, usize)>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < removed.len() {
        let first = removed[at];
        let mut last = first;
        while at + 1 < removed.len() && removed[at + 1] == last + 1 {
            at += 1;
            last = removed[at];
        }
        at += 1;
        if last + 1 < spans.len() {
            out.push((spans[first].0, spans[last + 1].0));
        } else {
            ensure!(first > 0, "the edit would remove every item of a list");
            out.push((spans[first - 1].1, spans[last].1));
        }
    }
    Ok(out)
}

/// The row's text with records put in and records taken out (by their index in
/// [`SiMain::records`]) and the count kept in step; nothing else changes.
pub fn edit_records(
    text: &[u8],
    main: &SiMain,
    insertions: &[Insertion],
    removals: &[usize],
) -> Result<Vec<u8>> {
    ensure!(
        !main.records.is_empty(),
        "an empty record list has no place to edit"
    );
    let mut ordered: Vec<&Insertion> = insertions.iter().collect();
    ordered.sort_by_key(|insertion| insertion.at);
    let total: usize = ordered
        .iter()
        .map(|insertion| insertion.records.len())
        .sum();
    let mut removed: Vec<usize> = removals.to_vec();
    removed.sort_unstable();
    removed.dedup();
    ensure!(
        removed.iter().all(|index| *index < main.records.len()),
        "a removal beyond the record list"
    );
    ensure!(
        removed.len() < main.records.len(),
        "the edit would remove every record"
    );
    let mut out = Vec::with_capacity(text.len() + total * 200);
    let mut copied = 0usize;
    let new_count = (main.records.len() + total - removed.len()).to_string();
    // (start, end, replacement); an insertion has start == end
    let mut edits: Vec<(usize, usize, Vec<u8>)> =
        vec![(main.count_span.0, main.count_span.1, new_count.into_bytes())];
    for insertion in ordered {
        ensure!(
            insertion.at <= main.records.len(),
            "an insertion beyond the record list"
        );
        let rendered = insertion
            .records
            .iter()
            .map(|record| record.render(main.eol))
            .collect::<Vec<_>>()
            .join(",");
        if insertion.at < main.records.len() {
            let at = main.records[insertion.at].start;
            edits.push((at, at, format!("{rendered},").into_bytes()));
        } else {
            let at = main.records[main.records.len() - 1].end;
            edits.push((at, at, format!(",{rendered}").into_bytes()));
        }
    }
    let spans: Vec<(usize, usize)> = main
        .records
        .iter()
        .map(|record| (record.start, record.end))
        .collect();
    for (start, end) in cuts(&spans, &removed)? {
        edits.push((start, end, Vec::new()));
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    for (start, end, replacement) in edits {
        ensure!(start >= copied, "overlapping edits");
        out.extend_from_slice(&text[copied..start]);
        out.extend_from_slice(&replacement);
        copied = end;
    }
    out.extend_from_slice(&text[copied..]);
    Ok(out)
}

/// `c4629235-....si`, the properties of the objects the platform looks up by uuid:
/// `{0,{<n>,<key>,<count>,(<property id>,<value>) x count,...}}`. The text with the
/// entries of `keys` (lower-case uuids) taken out and the entry count lowered;
/// nothing else changes. Returns the text and how many entries went. The
/// entries after a removed one keep their order: a removal needs nothing of the
/// hash order that an insertion would.
pub fn remove_property_entries(text: &[u8], keys: &BTreeSet<String>) -> Result<(Vec<u8>, usize)> {
    let mut scanner = Scanner::new(text);
    scanner.expect(b'{')?;
    let version = scanner.member()?;
    ensure!(
        atom(text, version)? == "0",
        "not a property row: it starts with {}",
        atom(text, version)?
    );
    ensure!(scanner.separator()?, "the property row has no body");
    let members = scanner.list()?;
    ensure!(
        !scanner.separator()?,
        "the property row has more than a body"
    );
    scanner.skip_ws();
    ensure!(
        scanner.pos == text.len(),
        "text after the property row at byte {}",
        scanner.pos
    );
    let count = atom(text, members[0])?
        .parse::<usize>()
        .map_err(|_| anyhow!("the property row has no entry count"))?;
    // entries: key, property count, then that many (id, value) pairs
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(count);
    let mut names: Vec<String> = Vec::with_capacity(count);
    let mut index = 1;
    while index < members.len() {
        ensure!(
            index + 1 < members.len(),
            "the property row ends inside an entry"
        );
        let key = atom(text, members[index])?.to_ascii_lowercase();
        let properties = atom(text, members[index + 1])?
            .parse::<usize>()
            .map_err(|_| anyhow!("an entry of the property row has no property count"))?;
        let end = index + 2 + 2 * properties;
        ensure!(
            end <= members.len(),
            "the property row ends inside the properties of {key}"
        );
        spans.push((members[index].0, members[end - 1].1));
        names.push(key);
        index = end;
    }
    ensure!(
        spans.len() == count,
        "the property row declares {count} entries and holds {}",
        spans.len()
    );
    let doomed: Vec<usize> = names
        .iter()
        .enumerate()
        .filter(|(_, key)| keys.contains(*key))
        .map(|(position, _)| position)
        .collect();
    if doomed.is_empty() {
        return Ok((text.to_vec(), 0));
    }
    ensure!(
        doomed.len() < spans.len(),
        "the edit would remove every entry"
    );
    let mut edits: Vec<(usize, usize, Vec<u8>)> = vec![(
        members[0].0,
        members[0].1,
        (count - doomed.len()).to_string().into_bytes(),
    )];
    for (start, end) in cuts(&spans, &doomed)? {
        edits.push((start, end, Vec::new()));
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = Vec::with_capacity(text.len());
    let mut copied = 0usize;
    for (start, end, replacement) in edits {
        ensure!(start >= copied, "overlapping edits");
        out.extend_from_slice(&text[copied..start]);
        out.extend_from_slice(&replacement);
        copied = end;
    }
    out.extend_from_slice(&text[copied..]);
    Ok((out, doomed.len()))
}

/// `siVersions`: `{0,<count>,"<name>",<version>,...}`; gives the entry named
/// `name` (case-insensitively) a new version.
pub fn set_si_version(text: &[u8], name: &str, version: Uuid) -> Result<Vec<u8>> {
    let mut scanner = Scanner::new(text);
    let members = scanner.list()?;
    scanner.skip_ws();
    ensure!(
        scanner.pos == text.len(),
        "text after siVersions at byte {}",
        scanner.pos
    );
    let head = atom(text, members[0])?;
    ensure!(head == "0", "siVersions starts with {head}, not 0");
    let count = atom(text, members[1])?
        .parse::<usize>()
        .map_err(|_| anyhow!("siVersions has no count"))?;
    ensure!(
        members.len() == 2 + 2 * count,
        "siVersions declares {count} entries and holds {} members",
        members.len() - 2
    );
    for pair in members[2..].chunks(2) {
        if unquote(text, pair[0])?.eq_ignore_ascii_case(name) {
            let mut out = Vec::with_capacity(text.len());
            out.extend_from_slice(&text[..pair[1].0]);
            out.extend_from_slice(version.hyphenated().to_string().as_bytes());
            out.extend_from_slice(&text[pair[1].1..]);
            return Ok(out);
        }
    }
    bail!("siVersions has no entry for {name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "66193438-abc5-410b-a1f1-a204102d1a62";
    const DP1: &str = "e983391b-e96a-4156-8d6d-b9f0be65c600";
    const DP2: &str = "0d0d0d0d-0000-4000-8000-000000000002";
    const DP3: &str = "0d0d0d0d-0000-4000-8000-000000000003";
    const F1: &str = "8d86192b-f421-4328-983d-40179c7aa2c9";
    const F2: &str = "f2f2f2f2-0000-4000-8000-000000000002";
    const T1: &str = "4f549963-efb2-4d60-9681-322a4740fb01";
    const C1: &str = "ba059c99-b392-42dd-8e4e-2e425601fce6";
    const A1: &str = "a1a1a1a1-0000-4000-8000-000000000001";

    // classes: 0 configuration, 1 data processor, 2 form, 3 template, 4 command, 5 attribute
    fn record(uuid: &str, parent: &str, kind: usize, name: &str, syn: &str) -> String {
        let block = if syn.is_empty() {
            "{1,0}".to_owned()
        } else {
            format!("{{1,1,\n{{\"ru\",\"{syn}\"}}\n}}")
        };
        format!("{uuid},{parent},{kind},\"{name}\",\n{block},0,0")
    }

    fn sample() -> String {
        let nil = "00000000-0000-0000-0000-000000000000";
        let records = [
            record(CFG, nil, 0, "Конфигурация", "Демо"),
            // DP1: forms, templates, commands (the full order)
            record(
                DP1,
                CFG,
                1,
                "ГенерацияШтрихкода",
                "Демо: Генерация штрихкода",
            ),
            record(F1, DP1, 2, "Форма", "Форма"),
            record(T1, DP1, 3, "Макет", "Макет"),
            record(C1, DP1, 4, "ГенерацияШтрихкода", ""),
            // DP2: only forms and commands
            record(DP2, CFG, 1, "Второй", "Второй"),
            record(
                "f3f3f3f3-0000-4000-8000-000000000003",
                DP2,
                2,
                "Форма",
                "Форма",
            ),
            record(
                "c3c3c3c3-0000-4000-8000-000000000003",
                DP2,
                4,
                "Команда",
                "Команда",
            ),
            // DP3: empty
            record(DP3, CFG, 1, "Пустой", "Пустой"),
        ];
        let classes = [
            "cf4abeab-37b2-11d4-940f-008048da11f9",
            "bf845118-327b-4682-b5c6-285d2a0eb296",
            "d5b0e5ed-256d-401c-9c36-f630cafd8a62",
            "3daea016-69b7-4ed4-9453-127911372fe6",
            "45556acb-826a-4f73-898a-6025fc9536e1",
            "ec6bb5e5-b7a8-4d75-bec9-658107a699cf",
        ];
        format!(
            "{{4,\n{{{},{}}},\n{{{},{}}}\n}}",
            classes.len(),
            classes.join(","),
            records.len(),
            records.join(",")
        )
    }

    #[test]
    fn a_row_parses_into_records_with_spans_and_subtrees() {
        let text = sample();
        let main = parse(text.as_bytes()).unwrap();
        assert_eq!(main.records.len(), 9);
        assert_eq!(main.classes.len(), 6);
        assert_eq!(main.records[1].name, "ГенерацияШтрихкода");
        let first = &main.records[2];
        assert!(text[first.start..first.end].starts_with(F1));
        assert!(text[first.start..first.end].ends_with(",0,0"));
        assert_eq!(main.subtree_end(1), 5, "DP1 owns records 2..5");
        assert_eq!(main.children(1), vec![2, 3, 4]);
        assert_eq!(main.subtree_end(0), 9, "the configuration owns everything");
        assert_eq!(
            main.kind_of_class("3DAEA016-69b7-4ed4-9453-127911372fe6"),
            Some(3)
        );
    }

    #[test]
    fn a_new_form_goes_behind_its_sibling_and_a_first_template_between_the_groups() {
        let main = parse(sample().as_bytes()).unwrap();
        // DP1 has a form: a new form goes behind it
        assert_eq!(main.place(DP1, 2, Some(F1), None).unwrap(), 3);
        // or in front of a later one
        assert_eq!(main.place(DP1, 2, None, Some(F1)).unwrap(), 2);
        // DP2 has forms and commands, no template: between them
        assert_eq!(main.place(DP2, 3, None, None).unwrap(), 7);
        // DP3 has nothing: right behind the owner
        assert_eq!(main.place(DP3, 3, None, None).unwrap(), 9);
        // the first form of DP2's kind... it has one; a first attribute goes in front of the forms
        assert!(
            main.place(DP2, 5, None, None).is_err(),
            "attribute order is not observed"
        );
        // a sibling that is not a child of the owner is refused
        assert!(main.place(DP2, 2, Some(F1), None).is_err());
    }

    #[test]
    fn records_are_inserted_in_place_and_the_count_follows() {
        let text = sample();
        let main = parse(text.as_bytes()).unwrap();
        let new_form = NewRecord {
            uuid: F2.to_owned(),
            parent: DP1.to_owned(),
            kind: 2,
            name: "ЯНоваяФормаApply".to_owned(),
            synonyms: vec![("ru".to_owned(), "Форма".to_owned())],
            flags: (0, 0),
        };
        let new_template = NewRecord {
            uuid: A1.to_owned(),
            parent: DP2.to_owned(),
            kind: 3,
            name: "Макет".to_owned(),
            synonyms: vec![
                ("ru".to_owned(), "Макет".to_owned()),
                ("en".to_owned(), "Template".to_owned()),
            ],
            flags: (0, 0),
        };
        let out = insert_records(
            text.as_bytes(),
            &main,
            &[
                Insertion {
                    at: 7,
                    records: vec![new_template.clone()],
                },
                Insertion {
                    at: 3,
                    records: vec![new_form.clone()],
                },
            ],
        )
        .unwrap();
        let again = parse(&out).unwrap();
        assert_eq!(again.records.len(), 11);
        let order: Vec<&str> = again.records.iter().map(|r| r.uuid.as_str()).collect();
        assert_eq!(order[3], F2, "behind the form of DP1, before its template");
        assert_eq!(order[4], T1);
        assert_eq!(order[8], A1, "between DP2's form and command");
        // everything else is byte for byte as it was
        let rendered = new_form.render(main.eol);
        let expected = text
            .replacen("{9,", "{11,", 1)
            .replacen(&format!("{T1},{DP1}"), &format!("{rendered},{T1},{DP1}"), 1)
            .replacen(
                "c3c3c3c3-0000-4000-8000-000000000003,",
                &format!(
                    "{},c3c3c3c3-0000-4000-8000-000000000003,",
                    new_template.render(main.eol)
                ),
                1,
            );
        assert_eq!(String::from_utf8(out).unwrap(), expected);
    }

    #[test]
    fn a_record_renders_as_the_platform_writes_it() {
        let record = NewRecord {
            uuid: "ade5fa33-4a02-5feb-b400-a41f22e208be".to_owned(),
            parent: "e983391b-e96a-4156-8d6d-b9f0be65c600".to_owned(),
            kind: 60,
            name: "ЯНоваяФормаApply".to_owned(),
            synonyms: vec![("ru".to_owned(), "Форма".to_owned())],
            flags: (0, 0),
        };
        assert_eq!(
            record.render("\n"),
            "ade5fa33-4a02-5feb-b400-a41f22e208be,e983391b-e96a-4156-8d6d-b9f0be65c600,60,\"ЯНоваяФормаApply\",\n{1,1,\n{\"ru\",\"Форма\"}\n},0,0"
        );
        // the platform's rows use CRLF
        assert_eq!(
            record.render("\r\n"),
            "ade5fa33-4a02-5feb-b400-a41f22e208be,e983391b-e96a-4156-8d6d-b9f0be65c600,60,\"ЯНоваяФормаApply\",\r\n{1,1,\r\n{\"ru\",\"Форма\"}\r\n},0,0"
        );
        // two languages: sorted by code, one pair per line
        let two = NewRecord {
            synonyms: vec![
                ("ru".to_owned(), "Форма".to_owned()),
                ("en".to_owned(), "Form".to_owned()),
            ],
            ..record.clone()
        };
        assert!(
            two.render("\n")
                .contains("\",\n{1,2,\n{\"en\",\"Form\"},\n{\"ru\",\"Форма\"}\n},0,0")
        );
        // no synonym at all
        let none = NewRecord {
            synonyms: Vec::new(),
            ..record
        };
        assert!(none.render("\n").contains("\",\n{1,0},0,0"));
    }

    #[test]
    fn a_row_with_crlf_and_a_bom_keeps_both() {
        let text = format!("\u{feff}{}", sample().replace('\n', "\r\n"));
        let main = parse(text.as_bytes()).unwrap();
        assert_eq!(main.eol, "\r\n");
        assert_eq!(main.records.len(), 9);
        let new_form = NewRecord {
            uuid: F2.to_owned(),
            parent: DP1.to_owned(),
            kind: 2,
            name: "Новая".to_owned(),
            synonyms: vec![("ru".to_owned(), "Новая".to_owned())],
            flags: (0, 0),
        };
        let out = insert_records(
            text.as_bytes(),
            &main,
            &[Insertion {
                at: 3,
                records: vec![new_form],
            }],
        )
        .unwrap();
        assert!(out.starts_with("\u{feff}{4,\r\n".as_bytes()));
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains(&format!(
            "{F2},{DP1},2,\"Новая\",\r\n{{1,1,\r\n{{\"ru\",\"Новая\"}}\r\n}},0,0,{T1},"
        )));
        assert_eq!(parse(out.as_bytes()).unwrap().records.len(), 10);
    }

    #[test]
    fn a_version_in_si_versions_is_replaced_in_place() {
        let text = "{0,2,\"aaaa.si\",11111111-1111-1111-1111-111111111111,\"BBBB.si\",22222222-2222-2222-2222-222222222222}";
        let new = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let out = set_si_version(text.as_bytes(), "bbbb.si", new).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "{0,2,\"aaaa.si\",11111111-1111-1111-1111-111111111111,\"BBBB.si\",33333333-3333-3333-3333-333333333333}"
        );
        assert!(set_si_version(text.as_bytes(), "cccc.si", new).is_err());
    }

    /// What the platform writes for the list without the records at `dropped`: the kept records joined
    /// with commas between the same head and tail, and the count.
    fn without(text: &str, main: &SiMain, dropped: &[usize]) -> String {
        let kept: Vec<&str> = main
            .records
            .iter()
            .enumerate()
            .filter(|(index, _)| !dropped.contains(index))
            .map(|(_, record)| &text[record.start..record.end])
            .collect();
        let head = &text[..main.records[0].start];
        let tail = &text[main.records[main.records.len() - 1].end..];
        format!("{head}{}{tail}", kept.join(",")).replacen(
            &format!("{{{},", main.records.len()),
            &format!("{{{},", kept.len()),
            1,
        )
    }

    #[test]
    fn records_are_taken_out_with_their_comma_and_the_count_follows() {
        let text = sample();
        let main = parse(text.as_bytes()).unwrap();
        for dropped in [
            vec![3usize],     // a template in the middle
            vec![2],          // the first form of the first owner
            vec![8],          // the last record: the comma in front of it goes
            vec![2, 3],       // a run in the middle
            vec![7, 8],       // a run that closes the list
            vec![3, 5, 8],    // several cuts
            vec![2, 4, 6, 7], // runs of one and two
        ] {
            let out = edit_records(text.as_bytes(), &main, &[], &dropped).unwrap();
            let expected = without(&text, &main, &dropped);
            assert_eq!(
                String::from_utf8(out.clone()).unwrap(),
                expected,
                "{dropped:?}"
            );
            let again = parse(&out).unwrap();
            assert_eq!(again.records.len(), 9 - dropped.len(), "{dropped:?}");
            for index in &dropped {
                assert!(again.index_of(&main.records[*index].uuid).is_none());
            }
        }
        // nothing to do is the same text
        assert_eq!(
            edit_records(text.as_bytes(), &main, &[], &[]).unwrap(),
            text.as_bytes()
        );
        // the platform's rows have a BOM and CRLF
        let crlf = format!("\u{feff}{}", sample().replace('\n', "\r\n"));
        let main = parse(crlf.as_bytes()).unwrap();
        let out = edit_records(crlf.as_bytes(), &main, &[], &[4, 7]).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            without(&crlf, &main, &[4, 7])
        );
    }

    #[test]
    fn a_removal_that_would_leave_no_record_or_names_none_is_refused() {
        let text = sample();
        let main = parse(text.as_bytes()).unwrap();
        assert!(edit_records(text.as_bytes(), &main, &[], &(0..9).collect::<Vec<_>>()).is_err());
        assert!(edit_records(text.as_bytes(), &main, &[], &[9]).is_err());
        // the first record with the rest is every record
        assert!(edit_records(text.as_bytes(), &main, &[], &[0, 1, 2, 3, 4, 5, 6, 7, 8]).is_err());
    }

    #[test]
    fn records_put_in_and_taken_out_in_one_pass_do_not_collide() {
        let text = sample();
        let main = parse(text.as_bytes()).unwrap();
        let new_form = NewRecord {
            uuid: F2.to_owned(),
            parent: DP1.to_owned(),
            kind: 2,
            name: "Новая".to_owned(),
            synonyms: vec![("ru".to_owned(), "Новая".to_owned())],
            flags: (0, 0),
        };
        // in front of a record that goes (T1 is 3)
        let out = edit_records(
            text.as_bytes(),
            &main,
            &[Insertion {
                at: 3,
                records: vec![new_form.clone()],
            }],
            &[3],
        )
        .unwrap();
        let again = parse(&out).unwrap();
        let order: Vec<&str> = again
            .records
            .iter()
            .map(|record| record.uuid.as_str())
            .collect();
        assert_eq!(order[..5], [CFG, DP1, F1, F2, C1]);
        assert_eq!(again.records.len(), 9);
        // at the end of the list, while the last record goes
        let out = edit_records(
            text.as_bytes(),
            &main,
            &[Insertion {
                at: 9,
                records: vec![new_form],
            }],
            &[8],
        )
        .unwrap();
        let again = parse(&out).unwrap();
        let order: Vec<&str> = again
            .records
            .iter()
            .map(|record| record.uuid.as_str())
            .collect();
        assert_eq!(order.len(), 9);
        assert_eq!(order[8], F2);
        assert_ne!(order[7], DP3);
    }

    const PROPS: &str = "\u{feff}{0,\r\n{4,\r\n\
        11111111-0000-4000-8000-000000000001,1,0,\r\n{\"S\",\"v8config://v8cfgHelp/mdobject/id11111111-0000-4000-8000-000000000001/x\"},\
        22222222-0000-4000-8000-000000000002,2,2,\r\n{\"N\",0},5,\r\n{\"B\",1},\
        33333333-0000-4000-8000-000000000003,1,3,\r\n{\"#\",fc01b5df-97fe-449b-83d4-218a090e681e,7},\
        44444444-0000-4000-8000-000000000004,1,0,\r\n{\"S\",\"y\"}\r\n}\r\n}";

    fn keys(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|key| (*key).to_owned()).collect()
    }

    #[test]
    fn property_entries_are_taken_out_and_the_others_keep_their_bytes_and_order() {
        let one = "11111111-0000-4000-8000-000000000001";
        let two = "22222222-0000-4000-8000-000000000002";
        let three = "33333333-0000-4000-8000-000000000003";
        let four = "44444444-0000-4000-8000-000000000004";
        // the first entry
        let (out, removed) = remove_property_entries(PROPS.as_bytes(), &keys(&[one])).unwrap();
        assert_eq!(removed, 1);
        let out = String::from_utf8(out).unwrap();
        assert!(out.starts_with("\u{feff}{0,\r\n{3,\r\n22222222-0000-4000-8000-000000000002,2,2,"));
        assert!(!out.contains(one));
        // the last one: the comma in front goes
        let (out, removed) = remove_property_entries(PROPS.as_bytes(), &keys(&[four])).unwrap();
        assert_eq!(removed, 1);
        let out = String::from_utf8(out).unwrap();
        assert!(out.ends_with("7}\r\n}\r\n}"), "{out:?}");
        assert!(out.contains("{3,"));
        // two neighbours and one more, in any order, and a key that is not there
        let (out, removed) = remove_property_entries(
            PROPS.as_bytes(),
            &keys(&[three, two, four, "55555555-0000-4000-8000-000000000005"]),
        )
        .unwrap();
        assert_eq!(removed, 3);
        let out = String::from_utf8(out).unwrap();
        assert!(out.starts_with("\u{feff}{0,\r\n{1,\r\n11111111-0000-4000-8000-000000000001,1,0,"));
        assert!(!out.contains(two) && !out.contains(three) && !out.contains(four));
        // a key that is not there: the same text
        let (out, removed) = remove_property_entries(
            PROPS.as_bytes(),
            &keys(&["55555555-0000-4000-8000-000000000005"]),
        )
        .unwrap();
        assert_eq!((removed, out.as_slice()), (0, PROPS.as_bytes()));
        // every entry is not an edit this row survives
        assert!(
            remove_property_entries(PROPS.as_bytes(), &keys(&[one, two, three, four])).is_err()
        );
        // an entry that runs past the row, and a row that is no property row
        assert!(remove_property_entries(b"{0,{2,a,1,0,{\"N\",0}}}", &keys(&["a"])).is_err());
        assert!(remove_property_entries(b"{4,{1,a,0}}", &keys(&["a"])).is_err());
    }
}
