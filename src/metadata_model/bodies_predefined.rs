//! Predefined data bodies, base-free: `Ext/Predefined.xml` and the owner's
//! own XML -> the stored row.
//!
//! - Catalog `<uuid>.1c`: `{0,<tree>}`, the tree's one root row `Элементы`
//!   holding the items;
//! - ChartOfCharacteristicTypes `<uuid>.7`: `{1,<tree>}`, root
//!   `Характеристики`;
//! - ChartOfAccounts `<uuid>.9`: `{2,<tree>}`, root `Счета`;
//! - ChartOfCalculationTypes `<uuid>.2`: a value table, one row per item.
//!
//! An object without `Ext/Predefined.xml` has no row: the export writes the
//! file for every row that holds an item, and the rows of the four corpora
//! without one are item tables emptied by deletions the source does not
//! record.
//!
//! What the source cannot give: the row indexes (a row keeps its index when
//! items are reordered or deleted) and the values an old root row carries
//! (a code and a description set by an earlier platform). The writer numbers
//! rows in document order and writes the root the way every freshly numbered
//! tree of the corpora stores it.

use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

use super::DescriptorContext;
use super::bodies_value_table::{
    Column, Row, bool_value, number_value, pattern, string_value, value_table, value_table_value,
    value_tree,
};
use super::brace::{Brace, NIL_UUID};
use super::types::{predefined_item_id, type_pattern};
use super::xml::{Element, parse_element_tree};
use crate::brace_list;

/// `{"#",<this>,{1,<uuid>}}`: a reference to a predefined item.
pub const PREDEFINED_REF_TYPE: &str = "ae135932-4f94-44df-92c1-c91f15a92848";
/// `{"#",<this>,{"Pattern",...}}`: a type description value.
const TYPE_DESCRIPTION_TYPE: &str = "f5c65050-3bbb-11d5-b988-0050bae0a95d";

/// The row suffix each predefined-data owner kind stores its items under.
pub fn predefined_suffix(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Catalog" => "1c",
        "ChartOfCharacteristicTypes" => "7",
        "ChartOfAccounts" => "9",
        "ChartOfCalculationTypes" => "2",
        _ => return None,
    })
}

/// `Ext/Predefined.xml` next to the owner's XML.
pub fn predefined_path(owner_xml: &Path) -> std::path::PathBuf {
    owner_xml
        .with_extension("")
        .join("Ext")
        .join("Predefined.xml")
}

/// The stored row of an owner's predefined data, `None` when it has none.
pub fn predefined_row(
    kind: &str,
    owner: &Element,
    owner_xml: &Path,
    context: &DescriptorContext,
) -> Result<Option<Brace>> {
    let path = predefined_path(owner_xml);
    if predefined_suffix(kind).is_none() || !context.source.source_file_exists(&path)? {
        return Ok(None);
    }
    let bytes = context
        .source
        .read_source(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let document = parse_element_tree(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let items = document.children_named("Item").collect::<Vec<_>>();
    predefined_tree(kind, owner, &items, false, context)
}

/// The row of an owner whose predefined items were all deleted: the tree
/// with its root alone. A configuration that had items keeps this row (its
/// `ConfigDumpInfo.xml` lists it) while the export writes no
/// `Predefined.xml` for it, so a load writes it from that list. A catalog's
/// root then carries the six values an edited tree stores -- the empty code
/// and description too -- as 10 of the 11 such rows of ERP УХ do (the
/// eleventh was never edited: four values). The next row index the platform
/// keeps (`-1,<n>` after the rows) counts the deleted items, which the
/// source does not record; the writer gives the fresh tree's 0.
pub fn emptied_predefined_row(
    kind: &str,
    owner: &Element,
    context: &DescriptorContext,
) -> Result<Option<Brace>> {
    if predefined_suffix(kind).is_none() {
        return Ok(None);
    }
    predefined_tree(kind, owner, &[], true, context)
}

fn predefined_tree(
    kind: &str,
    owner: &Element,
    items: &[&Element],
    emptied: bool,
    context: &DescriptorContext,
) -> Result<Option<Brace>> {
    let properties = owner
        .child("Properties")
        .ok_or_else(|| anyhow!("{kind} has no <Properties>"))?;
    let owner_name = properties.child_text("Name").unwrap_or_default();
    let row = match kind {
        "Catalog" => catalog_row(properties, items, emptied)?,
        "ChartOfCharacteristicTypes" => characteristic_row(properties, items, context)?,
        "ChartOfAccounts" => account_row(owner, owner_name, items, context)?,
        "ChartOfCalculationTypes" => calculation_row(properties, owner_name, items, context)?,
        _ => return Ok(None),
    };
    Ok(Some(row))
}

// ---------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------

fn predefined_ref(uuid: &str) -> Brace {
    brace_list![
        Brace::str("#"),
        Brace::atom(PREDEFINED_REF_TYPE),
        brace_list![Brace::num(1), Brace::uuid(uuid)],
    ]
}

fn nil_ref() -> Brace {
    predefined_ref(NIL_UUID)
}

fn ref_pattern() -> Brace {
    pattern(vec![brace_list![
        Brace::str("#"),
        Brace::atom(PREDEFINED_REF_TYPE)
    ]])
}

fn simple_pattern(code: &str) -> Brace {
    pattern(vec![brace_list![Brace::str(code)]])
}

fn text<'a>(element: &'a Element, name: &str) -> &'a str {
    element.child_text(name).unwrap_or_default()
}

fn number_prop(properties: &Element, name: &str) -> Result<i64> {
    let value = text(properties, name).trim();
    if value.is_empty() {
        return Ok(0);
    }
    value
        .parse::<i64>()
        .with_context(|| format!("<{name}> is not a number: {value:?}"))
}

fn flag(element: &Element, name: &str) -> bool {
    text(element, name).trim() == "true"
}

fn item_id(item: &Element) -> Result<String> {
    item.attr("id")
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| anyhow!("predefined <Item> without id"))
}

fn child_items(item: &Element) -> Vec<&Element> {
    item.child("ChildItems")
        .map(|children| children.children_named("Item").collect())
        .unwrap_or_default()
}

/// The owner's code column type: `{"S"}` / `{"S",<length>,<1 variable | 0
/// fixed>}` for a string code, `{"N",<length>,0,1}` for a number code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CodeType {
    String { length: i64, variable: bool },
    Number { length: i64 },
}

impl CodeType {
    fn of(properties: &Element) -> Result<Self> {
        let length = number_prop(properties, "CodeLength")?;
        Ok(match text(properties, "CodeType").trim() {
            "Number" => CodeType::Number { length },
            _ => CodeType::String {
                length,
                variable: text(properties, "CodeAllowedLength").trim() != "Fixed",
            },
        })
    }

    fn pattern(self) -> Brace {
        match self {
            CodeType::String { length: 0, .. } => simple_pattern("S"),
            CodeType::String { length, variable } => pattern(vec![brace_list![
                Brace::str("S"),
                Brace::num(length),
                Brace::flag(variable)
            ]]),
            CodeType::Number { length } => pattern(vec![brace_list![
                Brace::str("N"),
                Brace::num(length),
                Brace::num(0),
                Brace::num(1)
            ]]),
        }
    }

    /// The empty code: `{"S",""}`, a fixed-length string code padded to its
    /// length with spaces, or `{"N",0}`.
    fn empty(self) -> Brace {
        match self {
            CodeType::String {
                length,
                variable: false,
            } if length > 0 => string_value(&" ".repeat(length as usize)),
            CodeType::String { .. } => string_value(""),
            CodeType::Number { .. } => number_value("0"),
        }
    }

    fn length(self) -> i64 {
        match self {
            CodeType::String { length, .. } | CodeType::Number { length } => length,
        }
    }

    /// An item's `<Code>`.
    fn value(self, item: &Element) -> Brace {
        let code = item.child("Code");
        let spelled = code.map(|code| code.text.as_str()).unwrap_or_default();
        let decimal = code.and_then(|code| code.attr("type")) == Some("xs:decimal");
        match self {
            CodeType::Number { .. } if spelled.trim().is_empty() => number_value("0"),
            CodeType::Number { .. } => number_value(spelled.trim()),
            CodeType::String { .. } if decimal => number_value(spelled.trim()),
            CodeType::String { .. } => string_value(spelled),
        }
    }
}

/// `{"S",<length>,1}`, `{"S"}` when unlimited.
fn string_pattern(length: i64) -> Brace {
    if length == 0 {
        return simple_pattern("S");
    }
    pattern(vec![brace_list![
        Brace::str("S"),
        Brace::num(length),
        Brace::num(1)
    ]])
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

/// `{0,{1,<columns>,<rows>}}`: ref, IsFolder, parent, name, code,
/// description and a number column always 0. The root `Элементы` stores
/// four values, and a fifth, the empty code, when the code length is 0: 62
/// of the 63 freshly numbered such catalogs of ERP УХ and all 9 of BSP, and
/// 56 of the 77 with a code length. The other roots also store an empty
/// description (and a fixed code padded with spaces): every tree whose
/// items were reordered or deleted in the Designer stores six values, so
/// six is what an edit leaves, not what a load writes; an `emptied` tree
/// (every item deleted, see `emptied_predefined_row`) writes the six.
fn catalog_row(properties: &Element, items: &[&Element], emptied: bool) -> Result<Brace> {
    let code = CodeType::of(properties)?;
    let columns = vec![
        Column::new(0, "", ref_pattern(), 0),
        Column::new(1, "", simple_pattern("B"), 1),
        Column::new(2, "", ref_pattern(), 2),
        Column::new(3, "", simple_pattern("S"), 3),
        Column::new(4, "", code.pattern(), 4),
        Column::new(
            5,
            "",
            string_pattern(number_prop(properties, "DescriptionLength")?),
            5,
        ),
        Column::new(6, "", simple_pattern("N"), 6),
    ];
    let mut root_values = vec![
        nil_ref(),
        bool_value(true),
        nil_ref(),
        string_value("Элементы"),
    ];
    if emptied {
        root_values.push(code.empty());
        root_values.push(string_value(""));
    } else if code.length() == 0 {
        root_values.push(code.empty());
    }
    let mut root = Row::new(root_values);
    root.children = catalog_items(items, code)?;
    Ok(brace_list![Brace::num(0), value_tree(&columns, &[root])])
}

fn catalog_items(items: &[&Element], code: CodeType) -> Result<Vec<Row>> {
    items
        .iter()
        .map(|item| {
            let mut row = Row::new(vec![
                predefined_ref(&item_id(item)?),
                bool_value(flag(item, "IsFolder")),
                nil_ref(),
                string_value(text(item, "Name")),
                code.value(item),
                string_value(text(item, "Description")),
                number_value("0"),
            ]);
            row.children = catalog_items(&child_items(item), code)?;
            Ok(row)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ChartOfCharacteristicTypes
// ---------------------------------------------------------------------------

/// `{1,{1,<columns>,<rows>}}`: columns 1 to 7 are ref, IsFolder, name, code,
/// description, the value type and a number always 0. The root
/// `Характеристики` stores six values: the empty code (padded with spaces
/// for a fixed length), an empty description and the empty type.
fn characteristic_row(
    properties: &Element,
    items: &[&Element],
    context: &DescriptorContext,
) -> Result<Brace> {
    let code = CodeType::of(properties)?;
    let columns = vec![
        Column::new(1, "", ref_pattern(), 0),
        Column::new(2, "", simple_pattern("B"), 1),
        Column::new(3, "", simple_pattern("S"), 2),
        Column::new(4, "", code.pattern(), 3),
        Column::new(
            5,
            "",
            string_pattern(number_prop(properties, "DescriptionLength")?),
            4,
        ),
        Column::new(
            6,
            "",
            pattern(vec![brace_list![
                Brace::str("#"),
                Brace::atom(TYPE_DESCRIPTION_TYPE)
            ]]),
            5,
        ),
        Column::new(7, "", simple_pattern("N"), 6),
    ];
    let mut root = Row::new(vec![
        nil_ref(),
        bool_value(true),
        string_value("Характеристики"),
        code.empty(),
        string_value(""),
        type_description(brace_list![Brace::str("Pattern")]),
    ]);
    root.children = characteristic_items(items, code, context)?;
    Ok(brace_list![Brace::num(1), value_tree(&columns, &[root])])
}

fn type_description(pattern: Brace) -> Brace {
    brace_list![Brace::str("#"), Brace::atom(TYPE_DESCRIPTION_TYPE), pattern]
}

fn characteristic_items(
    items: &[&Element],
    code: CodeType,
    context: &DescriptorContext,
) -> Result<Vec<Row>> {
    items
        .iter()
        .map(|item| {
            let type_el = item.child("Type").map(canonical_type_prefixes);
            let mut row = Row::new(vec![
                predefined_ref(&item_id(item)?),
                bool_value(flag(item, "IsFolder")),
                string_value(text(item, "Name")),
                code.value(item),
                string_value(text(item, "Description")),
                type_description(type_pattern(type_el.as_ref(), context)?),
                number_value("0"),
            ]);
            row.children = characteristic_items(&child_items(item), code, context)?;
            Ok(row)
        })
        .collect()
}

/// A `<Type>` whose `v8:Type` names spell their namespace with a prefix of
/// their own (`xmlns:d4p1="...current-config"` -> `d4p1:CatalogRef.X`),
/// rewritten to the standard prefixes the type encoder reads (`cfg:`).
fn canonical_type_prefixes(type_el: &Element) -> Element {
    let mut out = type_el.clone();
    for child in &mut out.children {
        if !matches!(child.name.as_str(), "Type" | "TypeSet") {
            continue;
        }
        let trimmed = child.text.trim().to_string();
        let Some((prefix, local)) = trimmed.split_once(':') else {
            continue;
        };
        let declared = child
            .namespaces
            .iter()
            .chain(type_el.namespaces.iter())
            .find(|(name, _)| name == prefix)
            .map(|(_, uri)| uri.as_str());
        if let Some(standard) = declared.and_then(standard_prefix) {
            child.text = format!("{standard}:{local}");
        }
    }
    out
}

fn standard_prefix(uri: &str) -> Option<&'static str> {
    Some(match uri {
        "http://v8.1c.ru/8.1/data/enterprise/current-config" => "cfg",
        "http://www.w3.org/2001/XMLSchema" => "xs",
        "http://v8.1c.ru/8.1/data/core" => "v8",
        "http://v8.1c.ru/8.1/data/ui" => "v8ui",
        "http://v8.1c.ru/8.1/data/enterprise" => "ent",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// ChartOfAccounts
// ---------------------------------------------------------------------------

/// `{2,{1,<columns>,<rows>}}`: ref, name, code, description, account type,
/// off-balance, the extra dimension types (a value table), one boolean
/// column per accounting flag (named by the flag's uuid), the order (10000)
/// and a number always 0 (20000). A row stores the order and the 20000
/// number right after the dimension types and the flags last. The root
/// `Счета` stores the first seven values.
fn account_row(
    owner: &Element,
    owner_name: &str,
    items: &[&Element],
    context: &DescriptorContext,
) -> Result<Brace> {
    let properties = owner
        .child("Properties")
        .ok_or_else(|| anyhow!("ChartOfAccounts has no <Properties>"))?;
    let flags = child_object_ids(owner, "AccountingFlag")?;
    let dimension_flags = child_object_ids(owner, "ExtDimensionAccountingFlag")?;
    let mut columns = vec![
        Column::new(0, "", ref_pattern(), 0),
        Column::new(1, "", simple_pattern("S"), 1),
        Column::new(
            2,
            "",
            string_pattern(number_prop(properties, "CodeLength")?),
            2,
        ),
        Column::new(
            3,
            "",
            string_pattern(number_prop(properties, "DescriptionLength")?),
            3,
        ),
        Column::new(4, "", simple_pattern("N"), 4),
        Column::new(5, "", simple_pattern("B"), 5),
        Column::new(6, "", pattern(Vec::new()), 6),
    ];
    for (position, (uuid, _)) in flags.iter().enumerate() {
        columns.push(Column::new(
            7 + position as i64,
            uuid.clone(),
            simple_pattern("B"),
            9 + position as i64,
        ));
    }
    columns.push(Column::new(
        10000,
        "",
        string_pattern(number_prop(properties, "OrderLength")?),
        7,
    ));
    columns.push(Column::new(20000, "", simple_pattern("N"), 8));

    let chart = AccountChart {
        owner_name,
        flags: &flags,
        dimension_flags: &dimension_flags,
        context,
    };
    let mut root = Row::new(vec![
        nil_ref(),
        string_value("Счета"),
        string_value(""),
        string_value(""),
        number_value("0"),
        bool_value(false),
        chart.dimension_types(None)?,
    ]);
    root.children = chart.items(items)?;
    Ok(brace_list![Brace::num(2), value_tree(&columns, &[root])])
}

/// `(uuid, name)` of every child object of one element name, in order.
fn child_object_ids(owner: &Element, name: &str) -> Result<Vec<(String, String)>> {
    owner
        .child("ChildObjects")
        .into_iter()
        .flat_map(|children| children.children_named(name))
        .map(|child| {
            let uuid = child
                .attr("uuid")
                .ok_or_else(|| anyhow!("<{name}> without uuid"))?
                .to_ascii_lowercase();
            let child_name = child
                .path(&["Properties", "Name"])
                .map(|name| name.text.clone())
                .unwrap_or_default();
            Ok((uuid, child_name))
        })
        .collect()
}

struct AccountChart<'a> {
    owner_name: &'a str,
    flags: &'a [(String, String)],
    dimension_flags: &'a [(String, String)],
    context: &'a DescriptorContext,
}

const ACCOUNT_TYPES: &[(&str, &str)] = &[("Active", "0"), ("Passive", "1"), ("ActivePassive", "2")];

impl AccountChart<'_> {
    fn items(&self, items: &[&Element]) -> Result<Vec<Row>> {
        items
            .iter()
            .map(|item| {
                let account_type = text(item, "AccountType").trim();
                let account_type = ACCOUNT_TYPES
                    .iter()
                    .find_map(|(name, code)| (*name == account_type).then_some(*code))
                    .ok_or_else(|| anyhow!("unknown AccountType {account_type:?}"))?;
                let mut values = vec![
                    predefined_ref(&item_id(item)?),
                    string_value(text(item, "Name")),
                    string_value(text(item, "Code")),
                    string_value(text(item, "Description")),
                    number_value(account_type),
                    bool_value(flag(item, "OffBalance")),
                    self.dimension_types(item.child("ExtDimensionTypes"))?,
                    string_value(text(item, "Order")),
                    number_value("0"),
                ];
                let set = flag_values(
                    item.child("AccountingFlags"),
                    "AccountingFlag",
                    self.owner_name,
                );
                for (_, name) in self.flags {
                    values.push(bool_value(set.contains(&name.as_str())));
                }
                let mut row = Row::new(values);
                row.children = self.items(&child_items(item))?;
                Ok(row)
            })
            .collect()
    }

    /// The extra dimension types value table: the characteristic kind, the
    /// turnover flag, one column per extra-dimension accounting flag.
    fn dimension_types(&self, list: Option<&Element>) -> Result<Brace> {
        let mut columns = vec![
            Column::new(0, "", ref_pattern(), 0),
            Column::new(1, "", simple_pattern("B"), 1),
        ];
        for (position, (uuid, _)) in self.dimension_flags.iter().enumerate() {
            columns.push(Column::new(
                2 + position as i64,
                uuid.clone(),
                simple_pattern("B"),
                2 + position as i64,
            ));
        }
        let mut rows = Vec::new();
        for dimension in list
            .into_iter()
            .flat_map(|list| list.children_named("ExtDimensionType"))
        {
            let reference = dimension.attr("name").unwrap_or_default();
            let mut values = vec![
                predefined_ref(&predefined_reference_id(reference, self.context)?),
                bool_value(flag(dimension, "Turnover")),
            ];
            let set = flag_values(
                dimension.child("AccountingFlags"),
                "ExtDimensionAccountingFlag",
                self.owner_name,
            );
            for (_, name) in self.dimension_flags {
                values.push(bool_value(set.contains(&name.as_str())));
            }
            rows.push(Row::new(values));
        }
        Ok(value_table_value(&columns, &rows))
    }
}

/// The flag names an `<AccountingFlags>` list sets to `true`
/// (`<Flag ref="ChartOfAccounts.X.AccountingFlag.Name">true</Flag>`).
fn flag_values<'a>(list: Option<&'a Element>, kind: &str, owner_name: &str) -> Vec<&'a str> {
    let prefix = format!("ChartOfAccounts.{owner_name}.{kind}.");
    list.into_iter()
        .flat_map(|list| list.children_named("Flag"))
        .filter(|flag| flag.text.trim() == "true")
        .filter_map(|flag| flag.attr("ref")?.strip_prefix(prefix.as_str()))
        .collect()
}

/// `ChartOfCharacteristicTypes.X.Item` (or `...Calculation...`) -> the
/// predefined item's id; a bare uuid is itself.
fn predefined_reference_id(reference: &str, context: &DescriptorContext) -> Result<String> {
    let reference = reference.trim();
    if super::types::is_uuid(reference) {
        return Ok(reference.to_ascii_lowercase());
    }
    let parts = reference.splitn(3, '.').collect::<Vec<_>>();
    let [kind, name, item] = parts.as_slice() else {
        bail!("unsupported predefined item reference {reference:?}");
    };
    predefined_item_id(kind, name, item, context)
}

// ---------------------------------------------------------------------------
// ChartOfCalculationTypes
// ---------------------------------------------------------------------------

/// A value table, columns 1 to 9: ref, name, code, description, whether the
/// action period is the base, the displaced, base and leading calculation
/// types (each a value table of refs) and a number always 0.
fn calculation_row(
    properties: &Element,
    owner_name: &str,
    items: &[&Element],
    context: &DescriptorContext,
) -> Result<Brace> {
    let code = CodeType::of(properties)?;
    let list_pattern = pattern(vec![brace_list![
        Brace::str("#"),
        Brace::atom(super::bodies_value_table::VALUE_TABLE_TYPE)
    ]]);
    let columns = vec![
        Column::new(1, "", ref_pattern(), 0),
        Column::new(2, "", simple_pattern("S"), 1),
        Column::new(3, "", code.pattern(), 2),
        Column::new(
            4,
            "",
            string_pattern(number_prop(properties, "DescriptionLength")?),
            3,
        ),
        Column::new(5, "", simple_pattern("B"), 4),
        Column::new(6, "", list_pattern.clone(), 5),
        Column::new(7, "", list_pattern.clone(), 6),
        Column::new(8, "", list_pattern, 7),
        Column::new(9, "", simple_pattern("N"), 8),
    ];
    let mut rows = Vec::new();
    for item in items {
        rows.push(Row::new(vec![
            predefined_ref(&item_id(item)?),
            string_value(text(item, "Name")),
            code.value(item),
            string_value(text(item, "Description")),
            bool_value(flag(item, "ActionPeriodIsBase")),
            calculation_list(item.child("Displaced"), owner_name, context)?,
            calculation_list(item.child("Base"), owner_name, context)?,
            calculation_list(item.child("Leading"), owner_name, context)?,
            number_value("0"),
        ]));
    }
    Ok(value_table(&columns, &rows))
}

/// A list of calculation types: one ref column when it holds any, no
/// columns at all when empty.
fn calculation_list(
    list: Option<&Element>,
    _owner_name: &str,
    context: &DescriptorContext,
) -> Result<Brace> {
    let mut rows = Vec::new();
    for reference in list
        .into_iter()
        .flat_map(|list| list.children_named("CalculationType"))
    {
        rows.push(Row::new(vec![predefined_ref(&predefined_reference_id(
            &reference.text,
            context,
        )?)]));
    }
    let columns = if rows.is_empty() {
        Vec::new()
    } else {
        vec![Column::new(1, "", ref_pattern(), 0)]
    };
    Ok(value_table_value(&columns, &rows))
}
