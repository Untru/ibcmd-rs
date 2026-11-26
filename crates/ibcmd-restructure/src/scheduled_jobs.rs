//! Native scheduled-job tables and their extension service mapping.

use crate::brace::{Brace, parse_row, serialize};
use crate::brace_list;
use crate::schema::{FieldEntry, IndexEntry, TypeEntry};
use anyhow::{Context, Result, bail};
use ibcmd_schema::extension_schema::ExtensionScheduledJobsLayout as L;
use std::collections::BTreeSet;

fn list(items: Vec<Brace>) -> Brace {
    let mut all = vec![Brace::atom(items.len())];
    all.extend(items);
    Brace::List(all)
}

/// Each own scheduled job has a physical extension table. The schema model
/// does not claim to populate predefined instances: the caller rejects them
/// until their native ActivationCondition writer is implemented.
pub fn table_entry(number: u64) -> Brace {
    let fields = L::FIELDS
        .iter()
        .map(|(name, tag, a, b)| {
            FieldEntry::new(name, false, vec![TypeEntry::new(tag, *a, *b, "", 0)]).to_brace()
        })
        .collect();
    let index = IndexEntry {
        name: "ByID".into(),
        unique: true,
        fields: vec!["ID".into()],
        clustered: true,
    };
    brace_list![
        Brace::str(format!("{}{number}", L::TABLE_KIND)),
        Brace::str("N"),
        Brace::atom(number),
        Brace::str(""),
        list(fields),
        list(vec![]),
        list(vec![index.to_brace()]),
        Brace::num(1),
        Brace::str("S"),
        list(vec![]),
        list(vec![]),
        Brace::str(""),
        Brace::num(0),
        Brace::num(0)
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub uuid: String,
    pub table: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub extension: String,
    pub jobs: Vec<Job>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Service {
    pub groups: Vec<Group>,
}
fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}
fn table(value: &str) -> bool {
    value.strip_prefix(L::TABLE_KIND).is_some_and(|n| {
        !n.is_empty() && !n.starts_with('0') && n.bytes().all(|c| c.is_ascii_digit())
    })
}
fn count(value: Option<&Brace>) -> Result<usize> {
    value
        .and_then(Brace::as_atom)
        .context("missing service count")?
        .parse()
        .context("invalid service count")
}
fn atom(value: Option<&Brace>) -> Result<String> {
    Ok(value
        .and_then(Brace::as_atom)
        .context("missing service identity")?
        .to_ascii_lowercase())
}
impl Service {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(data)?
            .strip_prefix('\u{feff}')
            .unwrap_or(std::str::from_utf8(data)?);
        let (kind, body) = text
            .split_once(',')
            .context("scheduled-job service has no type prefix")?;
        if kind != L::PAYLOAD_TYPE {
            bail!("unknown scheduled-job service payload type");
        }
        let tree = parse_row(body.as_bytes())?;
        let items = tree
            .as_list()
            .context("scheduled-job service is not a list")?;
        let mut values = items.iter();
        let groups_count = count(values.next())?;
        if groups_count > 4096 {
            bail!("too many scheduled-job extension groups");
        }
        let mut groups = Vec::new();
        let mut extensions = BTreeSet::new();
        for _ in 0..groups_count {
            let extension = atom(values.next())?;
            if !uuid(&extension) || !extensions.insert(extension.clone()) {
                bail!("invalid or repeated scheduled-job extension identity");
            }
            let n = count(values.next())?;
            if n > 65536 {
                bail!("too many scheduled jobs");
            }
            let mut jobs = Vec::new();
            let mut ids = BTreeSet::new();
            let mut tables = BTreeSet::new();
            for _ in 0..n {
                let id = atom(values.next())?;
                let name = values
                    .next()
                    .and_then(Brace::as_str)
                    .context("missing scheduled-job table name")?
                    .to_owned();
                if !uuid(&id)
                    || !ids.insert(id.clone())
                    || !table(&name)
                    || !tables.insert(name.clone())
                {
                    bail!("invalid or repeated scheduled-job identity/table");
                }
                if values.next().and_then(Brace::as_atom) != Some("0") {
                    bail!("unmodeled scheduled-job service flags");
                }
                jobs.push(Job {
                    uuid: id,
                    table: name,
                });
            }
            groups.push(Group { extension, jobs });
        }
        if values.next().is_some() {
            bail!("scheduled-job service has trailing fields");
        }
        Ok(Self { groups })
    }
    pub fn with_group(&self, extension: &str, jobs: Vec<Job>) -> Result<Self> {
        if !uuid(extension) {
            bail!("invalid scheduled-job extension identity");
        }
        let mut out = self.clone();
        let group = Group {
            extension: extension.to_ascii_lowercase(),
            jobs,
        };
        if let Some(found) = out
            .groups
            .iter_mut()
            .find(|g| g.extension.eq_ignore_ascii_case(extension))
        {
            *found = group;
        } else {
            out.groups.push(group);
        }
        Self::parse(&out.to_bytes())
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut items = vec![Brace::atom(self.groups.len())];
        for group in &self.groups {
            items.push(Brace::atom(&group.extension));
            items.push(Brace::atom(group.jobs.len()));
            for job in &group.jobs {
                items.extend([
                    Brace::atom(&job.uuid),
                    Brace::str(&job.table),
                    Brace::num(0),
                ]);
            }
        }
        format!(
            "\u{feff}{},\r\n{}",
            L::PAYLOAD_TYPE,
            serialize(&Brace::List(items))
        )
        .into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_two_job_mapping_round_trips_exactly_and_preserves_other_groups() {
        let text = format!(
            "\u{feff}{},\r\n{{1,aaaaaaaa-0000-4000-8000-000000000001,2,aaaaaaaa-0000-4000-8000-000000000002,\"ScheduledJobs613\",0,aaaaaaaa-0000-4000-8000-000000000003,\"ScheduledJobs614\",0}}",
            L::PAYLOAD_TYPE
        );
        let service = Service::parse(text.as_bytes()).unwrap();
        assert_eq!(service.to_bytes(), text.as_bytes());
        let updated = service
            .with_group(
                "aaaaaaaa-0000-4000-8000-000000000009",
                vec![Job {
                    uuid: "aaaaaaaa-0000-4000-8000-000000000008".into(),
                    table: "ScheduledJobs615".into(),
                }],
            )
            .unwrap();
        assert_eq!(updated.groups[0], service.groups[0]);
        assert_eq!(updated.groups.len(), 2);
    }
    #[test]
    fn malformed_and_duplicate_job_mappings_refuse() {
        for body in [
            "{1,invalid,0}",
            "{1,aaaaaaaa-0000-4000-8000-000000000001,1,aaaaaaaa-0000-4000-8000-000000000002,\"ScheduledJobs1;DROP\",0}",
            "{0,extra}",
        ] {
            assert!(
                Service::parse(format!("\u{feff}{},\r\n{body}", L::PAYLOAD_TYPE).as_bytes())
                    .is_err()
            );
        }
        let bad = format!(
            "\u{feff}{},\r\n{{1,aaaaaaaa-0000-4000-8000-000000000001,2,aaaaaaaa-0000-4000-8000-000000000002,\"ScheduledJobs1\",0,aaaaaaaa-0000-4000-8000-000000000002,\"ScheduledJobs2\",0}}",
            L::PAYLOAD_TYPE
        );
        assert!(Service::parse(bad.as_bytes()).is_err());
    }
}
