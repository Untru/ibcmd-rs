//! Compose the original retained input admission and typed A1 observer. No implicit discovery.
use super::*;
use ibcmd_rs::restructure::caches::trace_diagnostic;

pub struct TypedCurrent {
    pub diagnostic: trace_diagnostic::Diagnostic,
    /// Same original source/case/stage/FileBinding roster, including unsupported descriptors.
    pub fact_sources: Vec<FactRowBinding>,
    pub key_sources: BTreeMap<String, Occurrence>,
    pub facts: FactGraph,
    pub comparison: comparison::CurrentComparison,
}

pub fn inspect(base: &ProjectionInputs, input: &FactInputs) -> Result<TypedCurrent> {
    inspect_with_proposal(base, input, None)
}

/// An explicit independent experiment retains the original closed, untrusted proposal protocol.
/// Neither observed edges nor native expected ordinals are converted automatically to a proposal.
pub fn inspect_with_proposal(
    base: &ProjectionInputs,
    input: &FactInputs,
    proposal: Option<&comparison::DiagnosticProposal>,
) -> Result<TypedCurrent> {
    input.verify_sources(base)?;
    let coverage = base.project()?;
    let facts = input.project(base)?;
    // Existing checked descriptor/type-index preflight runs before these canonical parses.
    // Bulk owner lookup, not a per-row scan or another parsed copy of the type-index row.
    let kinds: BTreeMap<_, _> = facts
        .descriptors
        .iter()
        .map(|fact| (fact.owner.as_str(), fact.kind))
        .collect();
    let mut rows = Vec::new();
    for (ordinal, row) in input.manifest.rows.iter().enumerate() {
        if let FactRole::Descriptor { owner } = &row.role {
            let owner_id = uuid(owner)?;
            if let Some(&kind) = kinds.get(owner_id.as_str()) {
                rows.push((
                    ordinal,
                    owner.as_str(),
                    kind,
                    parse_row(&input.plain[ordinal])?,
                ));
            }
            // Unsupported bodies remain in facts.partial and fact_sources, not a guessed map.
        }
    }
    drop(kinds);
    let descriptors = rows
        .iter()
        .map(|(ordinal, owner, kind, row)| trace_diagnostic::Descriptor {
            row_ordinal: *ordinal,
            owner,
            kind,
            row,
        })
        .collect::<Vec<_>>();
    let diagnostic = trace_diagnostic::inspect(&trace_diagnostic::Inputs {
        root: &base.manifest.registry_root_uuid,
        registry: &coverage.registry,
        sets: &coverage.sets,
        help: &coverage.help,
        type_index: facts.type_index.as_ref(),
        descriptors: &descriptors,
    })?;
    let key_sources = comparison::key_witnesses(base)?;
    ensure!(
        diagnostic.keys().len() == key_sources.len()
            && diagnostic
                .keys()
                .iter()
                .all(|key| key_sources.contains_key(&key.key)),
        "typed key/source witness bijection differs"
    );
    let comparison = comparison::compare_current(base, proposal)?;
    input.verify_sources(base)?;
    Ok(TypedCurrent {
        diagnostic,
        fact_sources: input.manifest.rows.clone(),
        key_sources,
        facts,
        comparison,
    })
}
