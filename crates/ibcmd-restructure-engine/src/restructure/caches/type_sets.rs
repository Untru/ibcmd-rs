//! `fe8acd6a-22c9-4b5a-aeae-232a1c8324cb.si`: sets of types.
//!
//! The row is `{0,{<n>,<set uuid>,{"Pattern",{"#",<type id>},...},...}}`: the platform's named sets of
//! type ids (all references to catalogs, all catalog objects, all references to anything, the
//! composite types a configuration defines...). The members are sorted as text. The sets of a kind
//! are constants of the platform: `e61ef7b8-...` holds the `Ref` type of every catalog, `e2cb8e3e-...`
//! the `Object` type, `280f5f0e-...` every reference type. A new object adds the type ids of its
//! generated types to the sets its kind belongs to.
//!
//! [`FAMILIES`] lists the sets by the *families* they are made of -- (kind, index of the generated
//! type in `2203278d`); `tests_corpus.rs` checks every line of it against the rows of the БСП corpus
//! (a set must be exactly the union of its families), so a wrong line cannot stay.

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};

/// `(set uuid, [(kind, generated type index)])`.
pub const FAMILIES: &[(&str, &[(&str, u32)])] = &[
    ("e61ef7b8-f3e1-4f4b-8ac7-676e90524997", &[("Catalog", 1)]),
    ("e2cb8e3e-31d7-4ebb-8cd2-3187c9586dce", &[("Catalog", 0)]),
    ("cc48cafe-e438-41a6-a8fc-b4e5abc0f313", &[("Constant", 1)]),
    ("38bfd075-3e63-4aaa-a93e-94521380d579", &[("Document", 1)]),
    ("f72bc2d7-45ee-4e28-84f8-87e13876a85b", &[("Document", 0)]),
    ("474c3bf6-08b5-4ddc-a2ad-989cedf11583", &[("Enum", 0)]),
    (
        "99892482-ed55-4fb5-a7f7-20888820a758",
        &[("ChartOfCharacteristicTypes", 1)],
    ),
    (
        "8f03014d-120b-4706-a0d8-d914e8fb56f3",
        &[("ChartOfCharacteristicTypes", 0)],
    ),
    (
        "ac606d60-0209-4159-8e4c-794bc091ce38",
        &[("ChartOfAccounts", 1)],
    ),
    (
        "71b5c8fd-60d6-4b3f-abd6-99e41a45d05c",
        &[("ChartOfAccounts", 0)],
    ),
    (
        "0a52f9de-73ea-4507-81e8-66217bead73a",
        &[("ExchangePlan", 1)],
    ),
    (
        "b07cb0a1-1ca9-4efd-8824-140e7c7fe683",
        &[("ExchangePlan", 0)],
    ),
    (
        "593cd424-0877-470d-91f9-b90a982059b4",
        &[("ChartOfCalculationTypes", 1)],
    ),
    (
        "58624d2c-d2af-4161-bd55-e2c2e2f2c0e6",
        &[("ChartOfCalculationTypes", 0)],
    ),
    // every reference type
    (
        "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63",
        &[
            ("Enum", 0),
            ("Catalog", 1),
            ("Document", 1),
            ("ExchangePlan", 1),
            ("ChartOfCharacteristicTypes", 1),
            ("BusinessProcess", 2),
            ("BusinessProcess", 10),
            ("Task", 2),
            ("ChartOfAccounts", 1),
            ("ChartOfCalculationTypes", 1),
        ],
    ),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeSets {
    /// `(set uuid, members)` in row order. A member is `{"#",<type id>}`, or a primitive type
    /// (`{"B"}`, `{"S",1024,1}`, `{"N",15,3,0}`) in the sets of composite types.
    pub sets: Vec<(String, Vec<Brace>)>,
}

/// The type id of a `{"#",<id>}` member.
pub fn member_id(member: &Brace) -> Option<&str> {
    match member.as_list()? {
        [tag, id] if tag.as_str() == Some("#") => id.as_atom(),
        _ => None,
    }
}

impl TypeSets {
    pub fn parse(text: &[u8]) -> Result<Self> {
        let tree = parse_row(text).context("fe8acd6a is not brace text")?;
        let outer = tree.as_list().context("fe8acd6a is not a list")?;
        let [zero, body] = outer else {
            bail!("fe8acd6a has {} elements, expected 2", outer.len());
        };
        if zero.as_atom() != Some("0") {
            bail!("fe8acd6a has version {zero:?}");
        }
        let body = body.as_list().context("fe8acd6a has no body")?;
        let (count, rest) = body.split_first().context("fe8acd6a body is empty")?;
        let count: usize = count
            .as_atom()
            .and_then(|count| count.parse().ok())
            .context("fe8acd6a has no set count")?;
        if rest.len() != 2 * count {
            bail!("fe8acd6a counts {count} sets but has {} items", rest.len());
        }
        let mut sets = Vec::with_capacity(count);
        for pair in rest.chunks(2) {
            let key = pair[0].as_atom().context("a set key is not a token")?;
            let pattern = pair[1].as_list().context("a set is not a list")?;
            let Some((head, members)) = pattern.split_first() else {
                bail!("a set is empty");
            };
            if head.as_str() != Some("Pattern") {
                bail!("a set does not start with \"Pattern\"");
            }
            if members.iter().any(|member| member.as_list().is_none()) {
                bail!("a member of a set is not a list");
            }
            sets.push((key.to_owned(), members.to_vec()));
        }
        Ok(Self { sets })
    }

    pub fn render(&self) -> Vec<u8> {
        let mut body = vec![Brace::num(self.sets.len() as i64)];
        for (key, members) in &self.sets {
            body.push(Brace::atom(key));
            let mut pattern = vec![Brace::str("Pattern")];
            pattern.extend(members.iter().cloned());
            body.push(Brace::List(pattern));
        }
        serialize_row(&Brace::List(vec![Brace::atom("0"), Brace::List(body)]))
    }

    /// Adds the generated types of a new object of `kind`. `types` are `(index, TypeId)`.
    /// A set that is not sorted is refused (only sorted sets are understood).
    pub fn add_object(&mut self, kind: &str, types: &[(u32, String)]) -> Result<()> {
        for (set, families) in FAMILIES {
            for (family_kind, index) in *families {
                if *family_kind != kind {
                    continue;
                }
                let Some((_, type_id)) = types.iter().find(|(candidate, _)| candidate == index)
                else {
                    continue;
                };
                let members = &mut self
                    .sets
                    .iter_mut()
                    .find(|(key, _)| key == set)
                    .with_context(|| format!("fe8acd6a has no set {set}"))?
                    .1;
                let ids: Option<Vec<&str>> = members.iter().map(member_id).collect();
                let ids = ids.with_context(|| {
                    format!("fe8acd6a set {set} has a member that is not a type id")
                })?;
                if !ids.windows(2).all(|pair| pair[0] < pair[1]) {
                    bail!("fe8acd6a set {set} is not sorted");
                }
                match ids.binary_search(&type_id.as_str()) {
                    Ok(_) => bail!("fe8acd6a set {set} has {type_id} already"),
                    Err(at) => {
                        members.insert(at, Brace::List(vec![Brace::str("#"), Brace::atom(type_id)]))
                    }
                }
            }
        }
        Ok(())
    }
}
