//! The check on rows: the active Config against the Config the ConfigSave
//! would make.
//!
//! The configuration is what its `versions` row lists: the names of all its
//! files. The ConfigSave holds the rows that were staged; a file it lists
//! but does not hold is unchanged, a file it no longer lists is gone, and a
//! file the active Config did not list is new. So the effective staged
//! configuration is the active Config overlaid by ConfigSave, restricted to
//! the files the staged `versions` lists.
//!
//! Every changed file is put in one of three boxes:
//! * a descriptor (a row without a suffix): decoded on both sides by the
//!   metadata model and compared property by property (`descriptor`);
//! * a body row (`<uuid>.<suffix>`): its role, from the owner's kind and the
//!   suffix, says whether applying it copies a row (a module, a form) or
//!   changes data the platform stores (predefined items, a flowchart);
//! * a service row (`root`, `version`, `versions`): the first two must not
//!   change at all.
//!
//! Whatever falls in none of them is a reason too: the check fails closed.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Result;

use crate::metadata_model::brace::parse_row;
use crate::metadata_model::objects::parts::Compat;
use crate::metadata_model::xml::Element;

use super::descriptor::{self, ObjectRef};
use super::model::{Note, ObjectOp, Reason, ReasonClass, Verdict};
use super::plan::{self, Decoder, Plan};
use super::roles::{BodyRole, Effect, RowName, body_role, parse_row_name};
use super::root_row;
use super::rule_id::RuleId;
use super::upgrade::RowProof;

/// Everything the check reads before it decides which body rows it needs.
#[derive(Default)]
pub struct Inputs {
    /// `2.20` or `2.21` to decode with; the Configuration row's shape when
    /// `None`.
    pub xml_version: Option<String>,
    /// The active Config: the row of every descriptor (uuid -> inflated).
    pub old_descriptors: BTreeMap<String, Vec<u8>>,
    pub old_root: Option<Vec<u8>>,
    pub old_version: Option<Vec<u8>>,
    /// The names of the files the active `versions` lists.
    pub old_inventory: BTreeSet<String>,
    /// The version id the active `versions` row gives each file, and the
    /// staged one. A file whose version changed must be in the ConfigSave.
    pub old_versions: BTreeMap<String, String>,
    pub new_versions: BTreeMap<String, String>,
    /// The ConfigSave: the rows of its descriptors.
    pub staged_descriptors: BTreeMap<String, Vec<u8>>,
    pub staged_root: Option<Vec<u8>>,
    pub staged_version: Option<Vec<u8>>,
    /// The `deleted` row a native import writes (`0` when it deleted no
    /// file).
    pub staged_deleted: Option<Vec<u8>>,
    /// Whether the ConfigSave holds a `versions` row, and what it lists.
    pub staged_has_versions: bool,
    pub new_inventory: BTreeSet<String>,
    /// The name of every row the ConfigSave holds.
    pub staged_names: BTreeSet<String>,
}

/// The rows the check asks for after it has placed the staged ones.
pub trait RowProvider {
    /// Inflated rows of the active Config, by published name; a name the
    /// Config does not hold is left out.
    fn old_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>>;
    /// Inflated rows of the ConfigSave, by name.
    fn staged_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>>;
}

/// Turns a stored descriptor into the element tree the diff compares.
pub trait Describe {
    fn describe_row(&self, kind: &str, row: &[u8]) -> Result<Element>;

    /// A row of the active configuration. It may be in a newer record format
    /// than the compatibility mode stores: a native apply promotes what the
    /// native import staged, and the object stays in that format.
    fn describe_stored(&self, kind: &str, row: &[u8]) -> Result<Element> {
        self.describe_row(kind, row)
    }

    /// A row of the staged configuration; the flag says it is in a record
    /// format the active configuration does not store.
    fn describe_staged(&self, kind: &str, row: &[u8]) -> Result<(Element, bool)> {
        self.describe_row(kind, row).map(|element| (element, false))
    }

    /// A staged row that decodes to the stored row's tree although its bytes
    /// differ: whether it differs by the record format only (`upgrade`). A
    /// describer that has no raw rows says [`RowProof::Unavailable`], and the
    /// check treats the row as one it cannot explain.
    fn prove_upgrade(&self, _kind: &str, _old: &[u8], _staged: &[u8]) -> RowProof {
        RowProof::Unavailable
    }
}

impl Describe for Decoder {
    fn describe_row(&self, kind: &str, row: &[u8]) -> Result<Element> {
        self.decode(kind, row)
    }

    fn describe_stored(&self, kind: &str, row: &[u8]) -> Result<Element> {
        self.decode_stored(kind, row)
    }

    fn describe_staged(&self, kind: &str, row: &[u8]) -> Result<(Element, bool)> {
        self.decode_staged(kind, row)
    }

    fn prove_upgrade(&self, kind: &str, old: &[u8], staged: &[u8]) -> RowProof {
        RowProof::of(kind, old, staged)
    }
}

fn reason(
    class: ReasonClass,
    rule: RuleId,
    object: &str,
    file_name: &str,
    property: &str,
    change: &str,
) -> Reason {
    Reason::step(class, rule, object, file_name, property, change)
}

fn effect_class(effect: Effect) -> Option<ReasonClass> {
    match effect {
        Effect::Safe => None,
        Effect::Structure => Some(ReasonClass::Structure),
        Effect::Data => Some(ReasonClass::Data),
    }
}

/// What the objects are called in messages.
pub struct Labels {
    names: HashMap<String, String>,
    children: HashMap<String, String>,
}

impl Labels {
    pub(super) fn new(new_plan: &Plan, old_plan: &Plan) -> Self {
        let mut names = HashMap::new();
        for plan in [old_plan, new_plan] {
            for (uuid, name) in plan.names.name_entries() {
                names.insert(uuid.to_string(), name.to_string());
            }
        }
        let mut children = HashMap::new();
        for plan in [old_plan, new_plan] {
            for (uuid, name) in &plan.children {
                children.insert(uuid.clone(), name.clone());
            }
        }
        Self { names, children }
    }

    pub(super) fn of(&self, uuid: &str) -> String {
        self.names
            .get(uuid)
            .or_else(|| self.children.get(uuid))
            .cloned()
            .unwrap_or_else(|| uuid.to_string())
    }
}

/// Runs the check.
pub fn check(inputs: &Inputs, rows: &dyn RowProvider) -> Verdict {
    let mut verdict = Verdict::new("rows");
    verdict.stats.old_files = inputs.old_inventory.len();
    verdict.stats.new_files = inputs.new_inventory.len();
    verdict.stats.staged_rows = inputs.staged_names.len();

    if inputs.staged_names.is_empty() {
        // Nothing staged: nothing to apply.
        return verdict;
    }
    let new_inventory = staged_inventory(inputs, &mut verdict);

    // The staged descriptors as they will be: the ConfigSave's, else the
    // active Config's, of the files the staged inventory lists.
    let mut new_descriptors = BTreeMap::<String, Vec<u8>>::new();
    for name in &new_inventory {
        if !matches!(parse_row_name(name), RowName::Descriptor(_)) {
            continue;
        }
        match inputs
            .staged_descriptors
            .get(name)
            .or_else(|| inputs.old_descriptors.get(name))
        {
            Some(bytes) => {
                new_descriptors.insert(name.clone(), bytes.clone());
            }
            None => {
                verdict.push_reason(reason(
                    ReasonClass::Unknown,
                    RuleId::VersionsListsUnknownFile,
                    name,
                    name,
                    "",
                    "the staged versions row lists a file that neither Config nor ConfigSave holds",
                ));
                unlisted(name, ObjectOp::Added, &mut verdict);
            }
        }
    }

    let load_old = |names: &[String]| rows.old_rows(names).unwrap_or_default();
    let mut old_plan = plan::build(
        &inputs.old_descriptors,
        inputs.old_root.as_deref(),
        inputs.xml_version.as_deref(),
        &load_old,
    );
    let load_new = |names: &[String]| {
        // A predefined body the ConfigSave holds wins over the Config's.
        let mut found = rows.old_rows(names).unwrap_or_default();
        let staged_names = names
            .iter()
            .filter(|name| inputs.staged_names.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        if !staged_names.is_empty() {
            found.extend(rows.staged_rows(&staged_names).unwrap_or_default());
        }
        found
    };
    let mut new_plan = plan::build(
        &new_descriptors,
        inputs.staged_root.as_deref().or(inputs.old_root.as_deref()),
        inputs.xml_version.as_deref(),
        &load_new,
    );
    let labels = Labels::new(&new_plan, &old_plan);
    plan_problems(&old_plan, &new_plan, &mut verdict);
    let mut names = std::mem::take(&mut new_plan.names);
    names.absorb(std::mem::take(&mut old_plan.names));
    // The active rows are read in the active compatibility mode, a staged row
    // in the staged one, or in the newest the model knows (a native import
    // upgrades the record format of the rows it stages).
    let decoder = Decoder::for_comparison(
        names,
        &new_plan.version,
        old_plan.compat,
        &[new_plan.compat, Compat(8, 3, 27)],
    );
    check_planned(
        inputs,
        rows,
        &old_plan,
        &new_plan,
        &labels,
        &decoder,
        &new_inventory,
        verdict,
    )
}

/// What went wrong while the descriptors were planned (a row that could not
/// be read, a name nobody could find): the kinds and names the rest of the
/// check relies on may be incomplete, so each problem is a reason.
pub(super) fn plan_problems(old_plan: &Plan, new_plan: &Plan, verdict: &mut Verdict) {
    let mut seen = BTreeSet::new();
    for error in old_plan.errors.iter().chain(new_plan.errors.iter()) {
        if seen.len() >= 10 || !seen.insert(error.clone()) {
            continue;
        }
        verdict.push_reason(reason(
            ReasonClass::Unknown,
            RuleId::DescriptorsUnreadable,
            "Configuration",
            "",
            "",
            &format!("the descriptors could not all be read: {error}"),
        ));
    }
}

/// The inventory of the staged configuration. A ConfigSave without a
/// `versions` row cannot say which files are gone: everything it holds is
/// taken for a change and nothing for a removal.
fn staged_inventory(inputs: &Inputs, verdict: &mut Verdict) -> BTreeSet<String> {
    if inputs.staged_has_versions {
        return inputs.new_inventory.clone();
    }
    verdict.push_reason(reason(
        ReasonClass::Unknown,
        RuleId::NoVersionsRow,
        "ConfigSave",
        "versions",
        "",
        "ConfigSave holds no versions row: the files of the staged configuration are unknown",
    ));
    let inventory = inputs
        .old_inventory
        .union(&inputs.staged_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    verdict.stats.new_files = inventory.len();
    inventory
}

/// The check once the descriptors are placed and can be decoded.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_planned(
    inputs: &Inputs,
    rows: &dyn RowProvider,
    old_plan: &Plan,
    new_plan: &Plan,
    labels: &Labels,
    describe: &dyn Describe,
    new_inventory: &BTreeSet<String>,
    mut verdict: Verdict,
) -> Verdict {
    // Service rows: the configuration and its compatibility mode must not
    // change.
    if let Some(staged) = &inputs.staged_root {
        let relation = match inputs.old_root.as_deref() {
            Some(old) => root_row::relation(old, staged),
            None => root_row::RootRelation::Different,
        };
        match relation {
            root_row::RootRelation::Same => {}
            // The 8.5 platform stamps the last block of the row's payload on every write.
            root_row::RootRelation::Restamped => verdict.push_note(Note {
                object: "Configuration".to_string(),
                file_name: "root".to_string(),
                property: String::new(),
                change: "the root row differs in the final block of its payload only: the 8.5 platform re-stamps it on every write"
                    .to_string(),
            }),
            root_row::RootRelation::Different => verdict.push_reason(reason(
                ReasonClass::Structure,
                RuleId::RootRowChanged,
                "Configuration",
                "root",
                "",
                "the root row differs: another configuration",
            )),
        }
    }
    if let Some(staged) = &inputs.staged_version
        && inputs.old_version.as_ref() != Some(staged)
    {
        verdict.push_reason(reason(
            ReasonClass::Structure,
            RuleId::VersionRowChanged,
            "Configuration",
            "version",
            "",
            "the version row differs: the compatibility mode or the features it lists changed",
        ));
    }

    let kind_of = |uuid: &str| -> Option<&'static str> {
        new_plan
            .kinds
            .get(uuid)
            .or_else(|| old_plan.kinds.get(uuid))
            .copied()
    };
    let owner_kind_of = |owner: &str| -> Option<&'static str> {
        plan::owner_kind(
            &new_plan.kinds,
            new_plan.module_group.as_deref(),
            &new_plan.children,
            owner,
        )
        .or_else(|| {
            plan::owner_kind(
                &old_plan.kinds,
                old_plan.module_group.as_deref(),
                &old_plan.children,
                owner,
            )
        })
    };

    // Every staged row.
    let mut to_compare = BTreeMap::<String, Option<BodyRole>>::new();
    for name in &inputs.staged_names {
        match parse_row_name(name) {
            // `versions` is the inventory; `root` and `version` were
            // compared above.
            RowName::Service(_) => {}
            RowName::DynamicMarker | RowName::Alias => verdict.push_reason(reason(
                ReasonClass::Unknown,
                RuleId::OnlineUpdateRow,
                name,
                name,
                "",
                "ConfigSave holds a row of the platform's own online-update state",
            )),
            RowName::Descriptor(_) => {
                if !new_inventory.contains(name) {
                    verdict.push_reason(reason(
                        ReasonClass::Unknown,
                        RuleId::DescriptorNotListed,
                        name,
                        name,
                        "",
                        "ConfigSave holds a descriptor that the staged versions row does not list",
                    ));
                    unlisted(name, ObjectOp::Changed, &mut verdict);
                    continue;
                }
                let Some(staged) = inputs.staged_descriptors.get(name) else {
                    verdict.stats.unreadable += 1;
                    let label = labels.of(name);
                    verdict.push_reason(reason(
                        ReasonClass::Unknown,
                        RuleId::StagedDescriptorUnreadable,
                        &label,
                        name,
                        "",
                        "the staged descriptor could not be read",
                    ));
                    unlisted_as(&label, name, ObjectOp::Changed, &mut verdict);
                    continue;
                };
                match inputs.old_descriptors.get(name) {
                    None => {
                        let label = labels.of(name);
                        match kind_of(name) {
                            Some(kind) => descriptor::lifecycle(
                                &ObjectRef {
                                    kind,
                                    name: &label,
                                    file_name: name,
                                    id: name,
                                },
                                true,
                                &mut verdict,
                            ),
                            None => {
                                verdict.push_reason(reason(
                                    ReasonClass::Unknown,
                                    RuleId::NewDescriptorUnplaced,
                                    &label,
                                    name,
                                    "",
                                    "a new descriptor of a kind the check cannot place",
                                ));
                                unlisted_as(&label, name, ObjectOp::Added, &mut verdict);
                            }
                        }
                    }
                    Some(old) if old == staged => {}
                    Some(old) => compare_descriptor(
                        name,
                        old,
                        staged,
                        &kind_of,
                        labels,
                        describe,
                        &mut verdict,
                    ),
                }
            }
            RowName::Body { owner, suffix } => {
                if !new_inventory.contains(name) {
                    verdict.push_reason(reason(
                        ReasonClass::Unknown,
                        RuleId::BodyRowNotListed,
                        &labels.of(owner),
                        name,
                        "",
                        "ConfigSave holds a row that the staged versions row does not list",
                    ));
                    continue;
                }
                let role = owner_kind_of(owner).and_then(|kind| body_role(kind, suffix));
                match role {
                    Some(role) if role.effect == Effect::Safe => {
                        *verdict
                            .stats
                            .body_rows_by_role
                            .entry(role.name.to_string())
                            .or_default() += 1;
                    }
                    other => {
                        to_compare.insert(name.clone(), other);
                    }
                }
            }
            // The row a native import writes to list the files it deleted:
            // `0` says there are none, and then it changes nothing. A list of
            // files is a removal the staged `versions` row must agree with;
            // nothing here has read one, so it is refused.
            RowName::Other if name == DELETED_ROW => {
                if !inputs.staged_deleted.as_deref().is_some_and(lists_no_file) {
                    verdict.push_reason(reason(
                        ReasonClass::Unknown,
                        RuleId::DeletedRowNotEmpty,
                        DELETED_ROW,
                        name,
                        "",
                        "the deleted row lists files (or could not be read): a removal is judged by the staged versions row, and this row is not one the check has read",
                    ));
                }
            }
            RowName::Other => verdict.push_reason(reason(
                ReasonClass::Unknown,
                RuleId::RowUnknown,
                name,
                name,
                "",
                "ConfigSave holds a row the check does not know",
            )),
        }
    }

    // The body rows that carry data (and the ones nobody could place):
    // compared by content.
    if !to_compare.is_empty() {
        let wanted = to_compare.keys().cloned().collect::<Vec<_>>();
        let old_rows = rows.old_rows(&wanted);
        let staged_rows = rows.staged_rows(&wanted);
        for (name, role) in &to_compare {
            verdict.stats.body_rows_compared += 1;
            let RowName::Body { owner, suffix } = parse_row_name(name) else {
                continue;
            };
            let label = labels.of(owner);
            let (class, role_name) = match role {
                Some(role) => (
                    effect_class(role.effect).unwrap_or(ReasonClass::Unknown),
                    role.name.to_string(),
                ),
                None => (ReasonClass::Unknown, format!("row .{suffix}")),
            };
            let (Ok(old_rows), Ok(staged_rows)) = (&old_rows, &staged_rows) else {
                verdict.stats.unreadable += 1;
                verdict.push_reason(reason(
                    class,
                    RuleId::BodyRowUnreadable,
                    &label,
                    name,
                    &role_name,
                    "the row could not be read",
                ));
                continue;
            };
            let Some(staged) = staged_rows.get(name) else {
                verdict.stats.unreadable += 1;
                verdict.push_reason(reason(
                    class,
                    RuleId::BodyRowUnreadable,
                    &label,
                    name,
                    &role_name,
                    "the staged row could not be read",
                ));
                continue;
            };
            match old_rows.get(name) {
                Some(old) if old == staged => {}
                Some(old) if content_relation(&role_name, old, staged).is_some() => {
                    verdict.push_note(Note {
                        object: label.clone(),
                        file_name: name.clone(),
                        property: role_name.clone(),
                        change: content_relation(&role_name, old, staged)
                            .unwrap_or_default()
                            .to_string(),
                    });
                }
                Some(old) => verdict.push_reason(reason(
                    class,
                    RuleId::BodyContentChanged,
                    &label,
                    name,
                    &role_name,
                    &format!("content changed ({} -> {} bytes)", old.len(), staged.len()),
                )),
                None => verdict.push_reason(reason(
                    class,
                    RuleId::BodyRowAdded,
                    &label,
                    name,
                    &role_name,
                    &format!("added ({} bytes)", staged.len()),
                )),
            }
        }
    }

    // A file the staged `versions` row gives another version, or that is new,
    // has to be in the ConfigSave: a stage that lists it and does not carry it
    // is partial, and applying it would leave the configuration inconsistent.
    for (name, version) in &inputs.new_versions {
        if inputs.staged_names.contains(name) || inputs.old_versions.is_empty() {
            continue;
        }
        let what = match inputs.old_versions.get(name) {
            Some(old) if old != version => "the staged versions row gives the file another version",
            None => "the staged versions row lists a new file",
            _ => continue,
        };
        let label = match parse_row_name(name) {
            RowName::Body { owner, .. } => labels.of(owner),
            _ => labels.of(name),
        };
        verdict.push_reason(reason(
            ReasonClass::Unknown,
            RuleId::StagedVersionsPartial,
            &label,
            name,
            "versions",
            &format!("{what}, but ConfigSave holds no row for it"),
        ));
    }

    // Files that are gone.
    for name in inputs.old_inventory.difference(new_inventory) {
        verdict.stats.removed_files += 1;
        match parse_row_name(name) {
            RowName::Descriptor(_) => {
                let label = labels.of(name);
                match kind_of(name) {
                    Some(kind) => descriptor::lifecycle(
                        &ObjectRef {
                            kind,
                            name: &label,
                            file_name: name,
                            id: name,
                        },
                        false,
                        &mut verdict,
                    ),
                    None => {
                        verdict.push_reason(reason(
                            ReasonClass::Unknown,
                            RuleId::RemovedDescriptorUnplaced,
                            &label,
                            name,
                            "",
                            "removed: a descriptor of a kind the check cannot place",
                        ));
                        unlisted_as(&label, name, ObjectOp::Removed, &mut verdict);
                    }
                }
            }
            RowName::Body { owner, suffix } => {
                let label = labels.of(owner);
                match owner_kind_of(owner).and_then(|kind| body_role(kind, suffix)) {
                    Some(role) => match effect_class(role.effect) {
                        None => verdict.push_note(Note {
                            object: label,
                            file_name: name.clone(),
                            property: role.name.to_string(),
                            change: "removed".to_string(),
                        }),
                        Some(class) => verdict.push_reason(reason(
                            class,
                            RuleId::BodyRowRemoved,
                            &label,
                            name,
                            role.name,
                            "removed",
                        )),
                    },
                    None => verdict.push_reason(reason(
                        ReasonClass::Unknown,
                        RuleId::BodyRowRemoved,
                        &label,
                        name,
                        &format!("row .{suffix}"),
                        "removed",
                    )),
                }
            }
            // `root`, `version` and `versions` are rows of the configuration,
            // not versioned files: the native import leaves them out of the
            // inventory it writes, ours lists them.
            RowName::Service(_) | RowName::DynamicMarker | RowName::Alias => {}
            RowName::Other => verdict.push_reason(reason(
                ReasonClass::Unknown,
                RuleId::RemovedRowUnknown,
                name,
                name,
                "",
                "removed: a row the check does not know",
            )),
        }
    }
    verdict.stats.added_files = new_inventory.difference(&inputs.old_inventory).count();
    verdict
}

/// The name of the row a native import lists the deleted files in.
const DELETED_ROW: &str = "deleted";

/// Whether the content of the `deleted` row is the empty list (`0`, with or
/// without the byte order mark).
fn lists_no_file(content: &[u8]) -> bool {
    let text = String::from_utf8_lossy(content);
    text.trim_start_matches('\u{feff}').trim() == "0"
}

/// The items of an exchange plan's content, order aside: the platform keeps
/// them in an order of its own, a stage writes them in another, and the
/// platform does not restructure for that (`{2,<n>,<uuid>,<flag>,...,0}`).
type ContentItems = (Vec<String>, Vec<(String, String)>, Vec<String>);

fn content_items(row: &[u8]) -> Option<ContentItems> {
    let text = std::str::from_utf8(row).ok()?;
    let tree = parse_row(text.strip_prefix('\u{feff}').unwrap_or(text).as_bytes()).ok()?;
    let items = tree.as_list()?;
    let atoms = items
        .iter()
        .map(|item| item.as_atom().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    let count = atoms.get(1)?.parse::<usize>().ok()?;
    if atoms.len() < 2 + 2 * count {
        return None;
    }
    let mut pairs = atoms[2..2 + 2 * count]
        .chunks(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect::<Vec<_>>();
    pairs.sort();
    Some((atoms[..2].to_vec(), pairs, atoms[2 + 2 * count..].to_vec()))
}

/// How the content of an exchange plan changed, when the platform lets it
/// pass: the same items in another order, or the same objects with other
/// flags of automatic registration (`AutoRecord`; seen to change no table).
/// `None`: an object joined or left, which creates or drops a registration
/// table, or something this reading does not understand.
pub(super) fn items_relation(
    old: &[(String, String)],
    new: &[(String, String)],
) -> Option<&'static str> {
    if old == new {
        return Some("the same items in another order");
    }
    let objects =
        |items: &[(String, String)]| items.iter().map(|item| item.0.clone()).collect::<Vec<_>>();
    (objects(old) == objects(new))
        .then_some("the same objects with other flags of automatic registration")
}

/// Whether two rows of a role that is compared by content differ in a way
/// that changes no table.
fn content_relation(role_name: &str, old: &[u8], staged: &[u8]) -> Option<&'static str> {
    if role_name != "Content" {
        return None;
    }
    let (old, staged) = (content_items(old)?, content_items(staged)?);
    if old.0 != staged.0 || old.2 != staged.2 {
        return None;
    }
    items_relation(&old.1, &staged.1)
}

/// An object nobody could place, listed by its row name.
fn unlisted(name: &str, op: ObjectOp, verdict: &mut Verdict) {
    unlisted_as(name, name, op, verdict);
}

fn unlisted_as(label: &str, name: &str, op: ObjectOp, verdict: &mut Verdict) {
    descriptor::unresolved(
        &ObjectRef {
            kind: "",
            name: label,
            file_name: name,
            id: name,
        },
        op,
        verdict,
    );
}

pub(super) fn compare_descriptor(
    name: &str,
    old: &[u8],
    staged: &[u8],
    kind_of: &dyn Fn(&str) -> Option<&'static str>,
    labels: &Labels,
    describe: &dyn Describe,
    verdict: &mut Verdict,
) {
    verdict.stats.descriptors_compared += 1;
    let label = labels.of(name);
    let Some(kind) = kind_of(name) else {
        verdict.push_reason(reason(
            ReasonClass::Unknown,
            RuleId::DescriptorKindUnknown,
            &label,
            name,
            "",
            "the descriptor changed and the check cannot tell what kind of object it is",
        ));
        unlisted_as(&label, name, ObjectOp::Changed, verdict);
        return;
    };
    let old_element = describe.describe_stored(kind, old);
    let new_element = describe.describe_staged(kind, staged);
    match (old_element, new_element) {
        (Ok(old_element), Ok((new_element, upgraded))) => {
            let count = descriptor::compare(
                &ObjectRef {
                    kind,
                    name: &label,
                    file_name: name,
                    id: name,
                },
                &old_element,
                &new_element,
                verdict,
            );
            if count == 0 {
                same_tree(
                    kind,
                    (name, &label),
                    (old, staged),
                    upgraded,
                    describe,
                    verdict,
                );
            }
        }
        (Err(error), _) | (_, Err(error)) => {
            verdict.stats.unreadable += 1;
            verdict.push_reason(reason(
                ReasonClass::Unknown,
                RuleId::RowUndecodable,
                &label,
                name,
                "",
                &format!("the {kind} row changed and cannot be decoded: {error:#}"),
            ));
            descriptor::unresolved(
                &ObjectRef {
                    kind,
                    name: &label,
                    file_name: name,
                    id: name,
                },
                ObjectOp::Changed,
                verdict,
            );
        }
    }
}

/// A staged descriptor whose bytes differ from the stored ones and that
/// decodes to the same tree: the decoder may not carry every field, so the
/// rows themselves are compared. Harmless only when the difference is the
/// record format of the platform that staged the row (`upgrade`); else a
/// reason that names the first place nothing explains.
fn same_tree(
    kind: &str,
    (name, label): (&str, &str),
    (old, staged): (&[u8], &[u8]),
    decoded_in_another_format: bool,
    describe: &dyn Describe,
    verdict: &mut Verdict,
) {
    let object = ObjectRef {
        kind,
        name: label,
        file_name: name,
        id: name,
    };
    let sizes = format!("{} -> {} bytes", old.len(), staged.len());
    match describe.prove_upgrade(kind, old, staged) {
        RowProof::Proven(_) => verdict.stats.format_upgrades += 1,
        RowProof::Refuted(deviation) => {
            verdict.push_reason(reason(
                ReasonClass::Unknown,
                RuleId::RowFormatUpgradeUnproven,
                label,
                name,
                "",
                &format!(
                    "the {kind} row differs ({sizes}) and decodes to the same XML,                      but the difference is not a known record-format upgrade: {}",
                    deviation.describe()
                ),
            ));
            descriptor::unresolved(&object, ObjectOp::Changed, verdict);
        }
        RowProof::Unavailable if decoded_in_another_format => {
            // The platform that imported the stage writes the record
            // format of its own edition; the descriptor is the same.
            verdict.push_note(Note {
                object: label.to_string(),
                file_name: name.to_string(),
                property: String::new(),
                change: "the row is in another record format; the descriptor is the same"
                    .to_string(),
            });
        }
        RowProof::Unavailable => {
            verdict.push_reason(reason(
                ReasonClass::Unknown,
                RuleId::RowDecodesToSameXml,
                label,
                name,
                "",
                &format!("the {kind} row differs ({sizes}) but both sides decode to the same XML"),
            ));
            descriptor::unresolved(&object, ObjectOp::Changed, verdict);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::export::NameIndex;
    use crate::metadata_model::objects::parts::Compat;
    use crate::metadata_model::xml::parse_element_tree;

    const CATALOG: &str = "6bc2c3f8-6027-411e-97c5-6f1b7e4cea61";
    const MODULE: &str = "ab132638-5188-470d-9432-de85f2b2c7d8";
    const NEW_CATALOG: &str = "11111111-1111-4111-8111-111111111111";
    const NEW_MODULE: &str = "22222222-2222-4222-8222-222222222222";
    const GROUP: &str = "f389d417-7804-4810-a1c5-a13b3209ad71";
    const PLAN: &str = "33333333-3333-4333-8333-333333333333";

    /// Rows that are XML: the "decoder" reads the row bytes as the element.
    struct Xml;

    impl Describe for Xml {
        fn describe_row(&self, _kind: &str, row: &[u8]) -> Result<Element> {
            parse_element_tree(row)
        }
    }

    #[derive(Default)]
    struct Rows {
        old: BTreeMap<String, Vec<u8>>,
        staged: BTreeMap<String, Vec<u8>>,
    }

    impl RowProvider for Rows {
        fn old_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
            Ok(names
                .iter()
                .filter_map(|name| self.old.get(name).map(|row| (name.clone(), row.clone())))
                .collect())
        }

        fn staged_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
            Ok(names
                .iter()
                .filter_map(|name| self.staged.get(name).map(|row| (name.clone(), row.clone())))
                .collect())
        }
    }

    fn plan_of(kinds: &[(&str, &'static str)]) -> Plan {
        Plan {
            configuration: None,
            module_group: Some(GROUP.to_string()),
            kinds: kinds
                .iter()
                .map(|(uuid, kind)| (uuid.to_string(), *kind))
                .collect(),
            children: HashMap::new(),
            names: NameIndex::default(),
            version: "2.20".to_string(),
            compat: Compat(8, 3, 27),
            errors: Vec::new(),
        }
    }

    fn labels() -> Labels {
        Labels {
            names: HashMap::from([
                (CATALOG.to_string(), "Catalog.Кассы".to_string()),
                (MODULE.to_string(), "CommonModule.Заметки".to_string()),
                (NEW_CATALOG.to_string(), "Catalog.Новый".to_string()),
                (NEW_MODULE.to_string(), "CommonModule.Новый".to_string()),
            ]),
            children: HashMap::new(),
        }
    }

    fn catalog_xml(code_length: &str, synonym: &str) -> Vec<u8> {
        format!(
            "<Catalog uuid=\"{CATALOG}\"><Properties><Name>Кассы</Name>\
             <Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>{synonym}</v8:content></v8:item></Synonym>\
             <CodeLength>{code_length}</CodeLength></Properties><ChildObjects/></Catalog>"
        )
        .into_bytes()
    }

    fn base_inputs() -> Inputs {
        let mut inputs = Inputs::default();
        inputs
            .old_descriptors
            .insert(CATALOG.to_string(), catalog_xml("9", "Кассы"));
        inputs
            .old_descriptors
            .insert(MODULE.to_string(), b"<CommonModule uuid=\"m\"><Properties><Global>false</Global></Properties></CommonModule>".to_vec());
        inputs.old_root = Some(b"root".to_vec());
        inputs.old_version = Some(b"version".to_vec());
        let inventory = [
            "root",
            "version",
            "versions",
            CATALOG,
            MODULE,
            &format!("{CATALOG}.0"),
            &format!("{CATALOG}.1c"),
            &format!("{MODULE}.0"),
        ]
        .iter()
        .map(|name| name.to_string())
        .collect::<BTreeSet<_>>();
        inputs.old_inventory = inventory.clone();
        inputs.new_inventory = inventory;
        inputs.staged_has_versions = true;
        inputs.staged_names.insert("versions".to_string());
        inputs
    }

    fn run(inputs: &Inputs, rows: &Rows) -> Verdict {
        let old = plan_of(&[
            (CATALOG, "Catalog"),
            (MODULE, "CommonModule"),
            (PLAN, "ExchangePlan"),
        ]);
        let new = plan_of(&[
            (CATALOG, "Catalog"),
            (MODULE, "CommonModule"),
            (PLAN, "ExchangePlan"),
            (NEW_CATALOG, "Catalog"),
            (NEW_MODULE, "CommonModule"),
        ]);
        let mut verdict = Verdict::new("rows");
        verdict.stats.staged_rows = inputs.staged_names.len();
        check_planned(
            inputs,
            rows,
            &old,
            &new,
            &labels(),
            &Xml,
            &inputs.new_inventory.clone(),
            std::mem::take(&mut verdict),
        )
    }

    fn stage(inputs: &mut Inputs, name: &str) {
        inputs.staged_names.insert(name.to_string());
    }

    #[test]
    fn a_module_and_a_form_row_are_copied_rows() {
        let mut inputs = base_inputs();
        stage(&mut inputs, &format!("{CATALOG}.0"));
        stage(&mut inputs, &format!("{MODULE}.0"));
        let verdict = run(&inputs, &Rows::default());
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
        assert_eq!(verdict.stats.body_rows_by_role["ObjectModule"], 1);
        assert_eq!(verdict.stats.body_rows_by_role["Module"], 1);
    }

    #[test]
    fn predefined_items_are_data_when_their_content_changes() {
        let mut inputs = base_inputs();
        let name = format!("{CATALOG}.1c");
        stage(&mut inputs, &name);
        let mut rows = Rows::default();
        rows.old.insert(name.clone(), b"{old}".to_vec());
        rows.staged.insert(name.clone(), b"{new items}".to_vec());
        let verdict = run(&inputs, &rows);
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons.len(), 1);
        let reason = &verdict.reasons[0];
        assert_eq!(reason.class, ReasonClass::Data);
        assert_eq!(reason.object, "Catalog.Кассы");
        assert_eq!(reason.property, "Predefined");
        assert_eq!(reason.file_name, name);

        // The same content is no change.
        rows.staged.insert(name.clone(), b"{old}".to_vec());
        assert!(!run(&inputs, &rows).needs_restructuring);
    }

    #[test]
    fn the_content_of_an_exchange_plan_is_compared_as_a_set() {
        const A: &str = "190bc52e-0d51-4dfc-9f16-99ed71ebfa75";
        const B: &str = "f81b51c5-57e4-41f8-9ad5-3d2717351b95";
        const C: &str = "1685c406-d0cb-4e0a-a3b0-036c8c22942a";
        let row = |items: &[(&str, &str)]| {
            let body = items
                .iter()
                .map(|(uuid, flag)| format!("{uuid},{flag}"))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{2,{},{body},0}}", items.len()).into_bytes()
        };
        let mut inputs = base_inputs();
        let name = format!("{PLAN}.1");
        inputs.old_inventory.insert(name.clone());
        inputs.new_inventory.insert(name.clone());
        stage(&mut inputs, &name);
        let mut rows = Rows::default();
        rows.old
            .insert(name.clone(), row(&[(A, "0"), (B, "0"), (C, "0")]));

        // The same items in the order the stage writes them.
        rows.staged
            .insert(name.clone(), row(&[(C, "0"), (A, "0"), (B, "0")]));
        let verdict = run(&inputs, &rows);
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
        assert_eq!(verdict.notes.len(), 1);
        assert_eq!(verdict.notes[0].property, "Content");

        // The same objects with another flag of automatic registration: no
        // table changes.
        rows.staged
            .insert(name.clone(), row(&[(C, "0"), (A, "1"), (B, "0")]));
        let verdict = run(&inputs, &rows);
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
        assert!(verdict.notes[0].change.contains("flags"));

        // Another object, one object less: registration tables come and go.
        for changed in [
            row(&[(C, "0"), (A, "0"), (PLAN, "0")]),
            row(&[(C, "0"), (A, "0")]),
        ] {
            rows.staged.insert(name.clone(), changed);
            let verdict = run(&inputs, &rows);
            assert!(verdict.needs_restructuring);
            assert_eq!(verdict.reasons[0].class, ReasonClass::Structure);
            assert_eq!(verdict.reasons[0].property, "Content");
        }

        // A row that is not a list of pairs is compared byte by byte.
        rows.old.insert(name.clone(), b"not a row".to_vec());
        rows.staged
            .insert(name.clone(), b"not a row either".to_vec());
        assert!(run(&inputs, &rows).needs_restructuring);
    }

    #[test]
    fn a_file_the_staged_versions_row_changes_has_to_be_staged() {
        let body = format!("{CATALOG}.0");
        let mut inputs = base_inputs();
        inputs.old_versions = BTreeMap::from([
            (CATALOG.to_string(), "v1".to_string()),
            (body.clone(), "v1".to_string()),
        ]);
        inputs.new_versions = BTreeMap::from([
            (CATALOG.to_string(), "v1".to_string()),
            (body.clone(), "v2".to_string()),
            (format!("{MODULE}.0"), "v1".to_string()),
        ]);
        // The catalog's module has another version and is not staged; the
        // common module's is a new file and is not staged either.
        let verdict = run(&inputs, &Rows::default());
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons.len(), 2, "{:?}", verdict.reasons);
        assert!(
            verdict
                .reasons
                .iter()
                .all(|r| r.class == ReasonClass::Unknown && r.property == "versions")
        );

        // Staged, both are in order.
        stage(&mut inputs, &body);
        stage(&mut inputs, &format!("{MODULE}.0"));
        let verdict = run(&inputs, &Rows::default());
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);

        // A stage that says nothing about the active versions is not judged by them.
        let mut inputs = base_inputs();
        inputs.new_versions = BTreeMap::from([(body, "v2".to_string())]);
        assert!(!run(&inputs, &Rows::default()).needs_restructuring);
    }

    #[test]
    fn a_body_row_of_an_unknown_role_is_unknown() {
        let mut inputs = base_inputs();
        let name = format!("{CATALOG}.77");
        inputs.old_inventory.insert(name.clone());
        inputs.new_inventory.insert(name.clone());
        stage(&mut inputs, &name);
        let mut rows = Rows::default();
        rows.old.insert(name.clone(), b"a".to_vec());
        rows.staged.insert(name.clone(), b"b".to_vec());
        let verdict = run(&inputs, &rows);
        assert_eq!(verdict.reasons.len(), 1);
        assert_eq!(verdict.reasons[0].class, ReasonClass::Unknown);
        assert_eq!(verdict.reasons[0].property, "row .77");
    }

    #[test]
    fn a_descriptor_change_goes_through_the_rules() {
        let mut inputs = base_inputs();
        stage(&mut inputs, CATALOG);
        inputs
            .staged_descriptors
            .insert(CATALOG.to_string(), catalog_xml("9", "Кассы (2)"));
        let verdict = run(&inputs, &Rows::default());
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
        assert_eq!(verdict.notes.len(), 1);
        assert_eq!(verdict.notes[0].object, "Catalog.Кассы");
        assert_eq!(verdict.notes[0].property, "Properties/Synonym/item/content");

        inputs
            .staged_descriptors
            .insert(CATALOG.to_string(), catalog_xml("12", "Кассы"));
        let verdict = run(&inputs, &Rows::default());
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons.len(), 1);
        assert_eq!(verdict.reasons[0].class, ReasonClass::Structure);
        assert_eq!(verdict.reasons[0].property, "Properties/CodeLength");
        assert!(verdict.reasons[0].change.starts_with("9 -> 12"));
        assert!(
            verdict
                .refusal()
                .unwrap()
                .starts_with("требуется штатный config apply: Catalog.Кассы")
        );
    }

    #[test]
    fn a_row_that_differs_but_decodes_to_the_same_tree_is_a_reason() {
        let mut inputs = base_inputs();
        stage(&mut inputs, CATALOG);
        let mut same = catalog_xml("9", "Кассы");
        same.extend_from_slice(b"  ");
        inputs.staged_descriptors.insert(CATALOG.to_string(), same);
        let verdict = run(&inputs, &Rows::default());
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons[0].class, ReasonClass::Unknown);
    }

    /// Reads a staged row in "another record format": the flag is set.
    struct Upgraded;

    impl Describe for Upgraded {
        fn describe_row(&self, _kind: &str, row: &[u8]) -> Result<Element> {
            parse_element_tree(row)
        }

        fn describe_staged(&self, _kind: &str, row: &[u8]) -> Result<(Element, bool)> {
            parse_element_tree(row).map(|element| (element, true))
        }
    }

    #[test]
    fn a_row_in_another_record_format_that_says_the_same_is_no_change() {
        let mut inputs = base_inputs();
        stage(&mut inputs, CATALOG);
        let mut same = catalog_xml("9", "Кассы");
        same.extend_from_slice(b"  ");
        inputs.staged_descriptors.insert(CATALOG.to_string(), same);
        let old = plan_of(&[(CATALOG, "Catalog"), (MODULE, "CommonModule")]);
        let mut verdict = Verdict::new("rows");
        verdict.stats.staged_rows = inputs.staged_names.len();
        let verdict = check_planned(
            &inputs,
            &Rows::default(),
            &old,
            &old,
            &labels(),
            &Upgraded,
            &inputs.new_inventory.clone(),
            verdict,
        );
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
        assert_eq!(verdict.notes.len(), 1);
        assert!(verdict.notes[0].change.contains("record format"));

        // A change in such a row is still a change.
        inputs
            .staged_descriptors
            .insert(CATALOG.to_string(), catalog_xml("12", "Кассы"));
        let verdict = check_planned(
            &inputs,
            &Rows::default(),
            &old,
            &old,
            &labels(),
            &Upgraded,
            &inputs.new_inventory.clone(),
            Verdict::new("rows"),
        );
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons[0].property, "Properties/CodeLength");
    }

    #[test]
    fn a_descriptor_that_cannot_be_decoded_is_unknown() {
        let mut inputs = base_inputs();
        stage(&mut inputs, CATALOG);
        inputs
            .staged_descriptors
            .insert(CATALOG.to_string(), b"<Catalog><unclosed>".to_vec());
        let verdict = run(&inputs, &Rows::default());
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons[0].class, ReasonClass::Unknown);
        assert_eq!(verdict.stats.unreadable, 1);
    }

    #[test]
    fn objects_added_to_the_configuration_are_judged_by_their_kind() {
        let mut inputs = base_inputs();
        for (uuid, xml) in [
            (NEW_CATALOG, "<Catalog uuid=\"n\"><Properties/></Catalog>"),
            (
                NEW_MODULE,
                "<CommonModule uuid=\"m2\"><Properties/></CommonModule>",
            ),
        ] {
            inputs.new_inventory.insert(uuid.to_string());
            stage(&mut inputs, uuid);
            inputs
                .staged_descriptors
                .insert(uuid.to_string(), xml.as_bytes().to_vec());
        }
        let verdict = run(&inputs, &Rows::default());
        assert_eq!(verdict.reasons.len(), 1, "{:?}", verdict.reasons);
        assert_eq!(verdict.reasons[0].object, "Catalog.Новый");
        assert!(verdict.reasons[0].change.starts_with("added"));
        assert_eq!(verdict.notes.len(), 1);
        assert_eq!(verdict.notes[0].object, "CommonModule.Новый");
        assert_eq!(verdict.stats.added_files, 2);
    }

    #[test]
    fn files_the_staged_inventory_drops_are_removed() {
        let mut inputs = base_inputs();
        stage(&mut inputs, "root");
        inputs.staged_root = Some(b"root".to_vec());
        inputs.new_inventory.remove(MODULE);
        inputs.new_inventory.remove(&format!("{MODULE}.0"));
        let verdict = run(&inputs, &Rows::default());
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
        assert_eq!(verdict.stats.removed_files, 2);
        assert_eq!(verdict.notes.len(), 2);

        inputs.new_inventory.remove(CATALOG);
        let verdict = run(&inputs, &Rows::default());
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons[0].object, "Catalog.Кассы");
        assert!(verdict.reasons[0].change.starts_with("removed"));
    }

    #[test]
    fn a_changed_root_or_version_row_is_a_reason() {
        let mut inputs = base_inputs();
        inputs.staged_root = Some(b"another".to_vec());
        stage(&mut inputs, "root");
        let verdict = run(&inputs, &Rows::default());
        assert_eq!(verdict.reasons.len(), 1);
        assert_eq!(verdict.reasons[0].file_name, "root");
        inputs.staged_root = None;
        inputs.staged_version = Some(b"other version".to_vec());
        stage(&mut inputs, "version");
        let verdict = run(&inputs, &Rows::default());
        assert_eq!(verdict.reasons.len(), 1);
        assert_eq!(verdict.reasons[0].file_name, "version");
    }

    #[test]
    fn a_root_row_that_the_8_5_platform_only_re_stamped_is_a_note() {
        // 32 bytes of payload (two blocks); the character at `at` sits in the second block for 36, in the first for 4
        let row = |at: usize, with: &str| {
            let mut payload = format!("{}=", "A".repeat(43));
            payload.replace_range(at..at + 1, with);
            format!("{{2,66193438-abc5-410b-a1f1-a204102d1a62,{payload}}}").into_bytes()
        };
        let mut inputs = base_inputs();
        inputs.old_root = Some(row(36, "A"));
        stage(&mut inputs, "root");

        inputs.staged_root = Some(row(36, "B"));
        let verdict = run(&inputs, &Rows::default());
        assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
        assert!(verdict.reasons.is_empty());
        assert_eq!(verdict.notes.len(), 1);
        assert_eq!(verdict.notes[0].file_name, "root");

        // the first block differs: another configuration
        inputs.staged_root = Some(row(4, "B"));
        let verdict = run(&inputs, &Rows::default());
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons.len(), 1);
        assert_eq!(verdict.reasons[0].rule, RuleId::RootRowChanged);
    }

    #[test]
    fn a_staged_row_the_inventory_does_not_list_or_the_check_does_not_know_is_unknown() {
        let mut inputs = base_inputs();
        stage(&mut inputs, &format!("{CATALOG}.9"));
        stage(&mut inputs, "Files.MobileVersions.dat");
        stage(&mut inputs, &format!("{MODULE}_dynupdate_{MODULE}"));
        let verdict = run(&inputs, &Rows::default());
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.reasons.len(), 3, "{:?}", verdict.reasons);
        assert!(
            verdict
                .reasons
                .iter()
                .all(|r| r.class == ReasonClass::Unknown)
        );
    }

    #[test]
    fn a_configsave_without_versions_cannot_tell_what_is_gone() {
        let mut inputs = base_inputs();
        inputs.staged_has_versions = false;
        inputs.staged_names.clear();
        stage(&mut inputs, &format!("{CATALOG}.0"));
        let mut verdict = Verdict::new("rows");
        let inventory = staged_inventory(&inputs, &mut verdict);
        assert!(verdict.needs_restructuring);
        assert_eq!(inventory.len(), inputs.old_inventory.len());
    }

    #[test]
    fn nothing_staged_is_nothing_to_apply() {
        let mut inputs = base_inputs();
        inputs.staged_names.clear();
        struct Never;
        impl RowProvider for Never {
            fn old_rows(&self, _: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
                panic!("nothing to read")
            }
            fn staged_rows(&self, _: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
                panic!("nothing to read")
            }
        }
        let verdict = check(&inputs, &Never);
        assert!(!verdict.needs_restructuring);
        assert_eq!(verdict.stats.staged_rows, 0);
    }
}
