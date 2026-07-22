//! Bounded deterministic planning over explicitly registered migration edges.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::artifact::ProfileId;
use crate::profile::{EffectiveProfile, ProfileRegistry};

use super::step::{MigrationStep, MigrationStepDescriptor, MigrationStepId};

/// Maximum profiles retained by one migration graph.
pub const MAX_MIGRATION_GRAPH_PROFILES: usize = 4_096;
/// Maximum registered directed steps retained by one migration graph.
pub const MAX_MIGRATION_GRAPH_STEPS: usize = 4_096;

/// Explicit semantic direction of one migration edge.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationDirection {
    /// Moves toward a newer or more capable explicitly named profile.
    Upgrade,
    /// Moves toward an older or less capable explicitly named profile.
    Downgrade,
    /// Moves between profiles without an asserted ordering.
    Lateral,
}

/// Evidence state attached to one registered migration edge.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationVerification {
    /// The edge exists for development but is not backed by verified evidence.
    Experimental,
    /// The edge is backed by the required offline compatibility evidence.
    Verified,
}

/// One object-safe step plus explicit direction and evidence metadata.
#[derive(Clone)]
pub struct MigrationEdge {
    step: Arc<dyn MigrationStep>,
    direction: MigrationDirection,
    verification: MigrationVerification,
}

impl MigrationEdge {
    /// Registers metadata without inferring direction from profile identifiers.
    pub fn new(
        step: Arc<dyn MigrationStep>,
        direction: MigrationDirection,
        verification: MigrationVerification,
    ) -> Self {
        Self {
            step,
            direction,
            verification,
        }
    }

    /// Returns the registered object-safe implementation.
    pub fn step(&self) -> &Arc<dyn MigrationStep> {
        &self.step
    }

    /// Returns the explicitly declared direction.
    pub const fn direction(&self) -> MigrationDirection {
        self.direction
    }

    /// Returns the explicitly declared evidence state.
    pub const fn verification(&self) -> MigrationVerification {
        self.verification
    }
}

impl Debug for MigrationEdge {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationEdge")
            .field("descriptor", self.step.descriptor())
            .field("direction", &self.direction)
            .field("verification", &self.verification)
            .finish()
    }
}

/// Endpoint named by a graph or plan error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationEndpoint {
    /// Source endpoint.
    Source,
    /// Target endpoint.
    Target,
}

impl Display for MigrationEndpoint {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source => "source",
            Self::Target => "target",
        })
    }
}

/// Stable failure while constructing a bounded migration graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationGraphError {
    /// The profile registry exceeds the graph bound.
    TooManyProfiles {
        /// Maximum accepted profiles.
        maximum: usize,
        /// Actual profiles.
        actual: usize,
    },
    /// Registered steps exceed the graph bound.
    TooManySteps {
        /// Maximum accepted steps.
        maximum: usize,
        /// Actual steps.
        actual: usize,
    },
    /// The same implementation was registered twice as the same exact edge.
    DuplicateEdge {
        /// Stable step identifier.
        step_id: MigrationStepId,
        /// Exact source profile.
        source_profile: ProfileId,
        /// Exact target profile.
        target_profile: ProfileId,
    },
    /// Two distinct implementations declare the same stable step identifier.
    DuplicateStepId {
        /// Duplicate identifier.
        step_id: MigrationStepId,
    },
    /// A descriptor endpoint is absent from the exact profile registry.
    UnknownEdgeProfile {
        /// Step containing the invalid endpoint.
        step_id: MigrationStepId,
        /// Invalid endpoint.
        endpoint: MigrationEndpoint,
        /// Missing exact profile.
        profile: ProfileId,
    },
    /// A registered effective profile does not satisfy an endpoint constraint.
    ConstraintMismatch {
        /// Step containing the unsatisfied constraint.
        step_id: MigrationStepId,
        /// Unsatisfied endpoint.
        endpoint: MigrationEndpoint,
        /// Exact profile named by the constraint.
        profile: ProfileId,
    },
    /// A self-loop cannot make migration progress and is an invalid cycle.
    Cycle {
        /// Self-looping step.
        step_id: MigrationStepId,
        /// Repeated exact profile.
        profile: ProfileId,
    },
}

impl MigrationGraphError {
    /// Returns a stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TooManyProfiles { .. } => "migration.graph-too-many-profiles",
            Self::TooManySteps { .. } => "migration.graph-too-many-steps",
            Self::DuplicateEdge { .. } => "migration.graph-duplicate-edge",
            Self::DuplicateStepId { .. } => "migration.graph-duplicate-step-id",
            Self::UnknownEdgeProfile { .. } => "migration.graph-unknown-profile",
            Self::ConstraintMismatch { .. } => "migration.graph-constraint-mismatch",
            Self::Cycle { .. } => "migration.graph-cycle",
        }
    }
}

impl Display for MigrationGraphError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyProfiles { maximum, actual } => {
                write!(
                    formatter,
                    "migration graph exceeds {maximum} profiles (actual {actual})"
                )
            }
            Self::TooManySteps { maximum, actual } => {
                write!(
                    formatter,
                    "migration graph exceeds {maximum} steps (actual {actual})"
                )
            }
            Self::DuplicateEdge {
                step_id,
                source_profile,
                target_profile,
            } => write!(
                formatter,
                "migration edge `{step_id}` from `{source_profile}` to `{target_profile}` was registered twice"
            ),
            Self::DuplicateStepId { step_id } => {
                write!(formatter, "duplicate migration step identifier `{step_id}`")
            }
            Self::UnknownEdgeProfile {
                step_id,
                endpoint,
                profile,
            } => write!(
                formatter,
                "migration step `{step_id}` has unknown {endpoint} profile `{profile}`"
            ),
            Self::ConstraintMismatch {
                step_id,
                endpoint,
                profile,
            } => write!(
                formatter,
                "migration step `{step_id}` {endpoint} constraint does not match profile `{profile}`"
            ),
            Self::Cycle { step_id, profile } => write!(
                formatter,
                "migration step `{step_id}` forms a self-loop at profile `{profile}`"
            ),
        }
    }
}

impl Error for MigrationGraphError {}

/// Stable failure to plan one exact route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationPlanError {
    /// An exact endpoint is absent from the graph registry.
    UnknownEndpoint {
        /// Missing endpoint role.
        endpoint: MigrationEndpoint,
        /// Missing exact profile.
        profile: ProfileId,
    },
    /// The supplied endpoint differs from the graph's exact effective profile.
    EndpointDefinitionMismatch {
        /// Mismatched endpoint role.
        endpoint: MigrationEndpoint,
        /// Exact profile identifier.
        profile: ProfileId,
    },
    /// No directed route exists between the exact endpoints.
    NoPath {
        /// Exact source profile.
        source_profile: ProfileId,
        /// Exact target profile.
        target_profile: ProfileId,
    },
    /// Every route requires at least one unverified downgrade edge.
    UnverifiedDowngradeRequired {
        /// Exact source profile.
        source_profile: ProfileId,
        /// Exact target profile.
        target_profile: ProfileId,
        /// Sorted blocked steps that lie on a directed source-to-target path.
        blocked_steps: Vec<MigrationStepId>,
    },
    /// More than one equally short valid route exists.
    AmbiguousShortestPath {
        /// Exact source profile.
        source_profile: ProfileId,
        /// Exact target profile.
        target_profile: ProfileId,
        /// Shared shortest route length.
        steps: usize,
    },
}

impl MigrationPlanError {
    /// Returns a stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownEndpoint { .. } => "migration.plan-unknown-endpoint",
            Self::EndpointDefinitionMismatch { .. } => {
                "migration.plan-endpoint-definition-mismatch"
            }
            Self::NoPath { .. } => "migration.plan-no-path",
            Self::UnverifiedDowngradeRequired { .. } => "migration.plan-unverified-downgrade",
            Self::AmbiguousShortestPath { .. } => "migration.plan-ambiguous-shortest-path",
        }
    }
}

impl Display for MigrationPlanError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEndpoint { endpoint, profile } => {
                write!(
                    formatter,
                    "unknown migration {endpoint} profile `{profile}`"
                )
            }
            Self::EndpointDefinitionMismatch { endpoint, profile } => write!(
                formatter,
                "migration {endpoint} profile `{profile}` differs from the registered effective profile"
            ),
            Self::NoPath {
                source_profile,
                target_profile,
            } => write!(
                formatter,
                "no migration path exists from `{source_profile}` to `{target_profile}`"
            ),
            Self::UnverifiedDowngradeRequired {
                source_profile,
                target_profile,
                blocked_steps,
            } => write!(
                formatter,
                "migration from `{source_profile}` to `{target_profile}` requires one of {} unverified downgrade steps",
                blocked_steps.len()
            ),
            Self::AmbiguousShortestPath {
                source_profile,
                target_profile,
                steps,
            } => write!(
                formatter,
                "migration from `{source_profile}` to `{target_profile}` has multiple shortest paths of {steps} steps"
            ),
        }
    }
}

impl Error for MigrationPlanError {}

/// One immutable deterministic route produced by [`MigrationGraph::plan`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationPlan {
    source_profile: ProfileId,
    target_profile: ProfileId,
    route_profiles: Vec<ProfileId>,
    step_ids: Vec<MigrationStepId>,
}

impl MigrationPlan {
    /// Returns the exact route source.
    pub const fn source_profile(&self) -> &ProfileId {
        &self.source_profile
    }

    /// Returns the exact route target.
    pub const fn target_profile(&self) -> &ProfileId {
        &self.target_profile
    }

    /// Returns source, intermediate, and target profiles in route order.
    pub fn route_profiles(&self) -> &[ProfileId] {
        &self.route_profiles
    }

    /// Returns step identifiers in execution order.
    pub fn step_ids(&self) -> &[MigrationStepId] {
        &self.step_ids
    }

    /// Returns whether source and target are the same exact profile.
    pub const fn is_empty(&self) -> bool {
        self.step_ids.is_empty()
    }

    /// Returns the number of steps in this bounded route.
    pub const fn len(&self) -> usize {
        self.step_ids.len()
    }
}

pub(crate) struct RegisteredStep {
    implementation: Arc<dyn MigrationStep>,
    descriptor: MigrationStepDescriptor,
    direction: MigrationDirection,
    verification: MigrationVerification,
}

/// Bounded graph of exact profiles and immutable registered step contracts.
pub struct MigrationGraph {
    profiles: BTreeMap<ProfileId, EffectiveProfile>,
    steps: BTreeMap<MigrationStepId, RegisteredStep>,
    outgoing: BTreeMap<ProfileId, Vec<MigrationStepId>>,
}

impl Debug for MigrationGraph {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationGraph")
            .field("profiles", &self.profiles.keys().collect::<Vec<_>>())
            .field("steps", &self.steps.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl MigrationGraph {
    /// Validates and snapshots a deterministic registry of object-safe steps.
    pub fn new(
        profiles: &ProfileRegistry,
        edges: Vec<MigrationEdge>,
    ) -> Result<Self, MigrationGraphError> {
        if profiles.profiles().len() > MAX_MIGRATION_GRAPH_PROFILES {
            return Err(MigrationGraphError::TooManyProfiles {
                maximum: MAX_MIGRATION_GRAPH_PROFILES,
                actual: profiles.profiles().len(),
            });
        }
        if edges.len() > MAX_MIGRATION_GRAPH_STEPS {
            return Err(MigrationGraphError::TooManySteps {
                maximum: MAX_MIGRATION_GRAPH_STEPS,
                actual: edges.len(),
            });
        }

        let profile_map = profiles.profiles().clone();
        let mut steps = BTreeMap::<MigrationStepId, RegisteredStep>::new();
        let mut outgoing = BTreeMap::<ProfileId, Vec<MigrationStepId>>::new();
        let mut implementations = BTreeSet::<*const ()>::new();

        for edge in edges {
            let descriptor = edge.step.descriptor().clone();
            let step_id = descriptor.id().clone();
            let source_profile = descriptor.source().exact_profile().clone();
            let target_profile = descriptor.target().exact_profile().clone();

            let implementation_identity = Arc::as_ptr(&edge.step) as *const ();
            if !implementations.insert(implementation_identity) {
                return Err(MigrationGraphError::DuplicateEdge {
                    step_id,
                    source_profile,
                    target_profile,
                });
            }
            if steps.contains_key(&step_id) {
                return Err(MigrationGraphError::DuplicateStepId { step_id });
            }

            let source = profile_map.get(&source_profile).ok_or_else(|| {
                MigrationGraphError::UnknownEdgeProfile {
                    step_id: step_id.clone(),
                    endpoint: MigrationEndpoint::Source,
                    profile: source_profile.clone(),
                }
            })?;
            let target = profile_map.get(&target_profile).ok_or_else(|| {
                MigrationGraphError::UnknownEdgeProfile {
                    step_id: step_id.clone(),
                    endpoint: MigrationEndpoint::Target,
                    profile: target_profile.clone(),
                }
            })?;
            if !descriptor.source().matches(source) {
                return Err(MigrationGraphError::ConstraintMismatch {
                    step_id,
                    endpoint: MigrationEndpoint::Source,
                    profile: source_profile,
                });
            }
            if !descriptor.target().matches(target) {
                return Err(MigrationGraphError::ConstraintMismatch {
                    step_id,
                    endpoint: MigrationEndpoint::Target,
                    profile: target_profile,
                });
            }
            if source_profile == target_profile {
                return Err(MigrationGraphError::Cycle {
                    step_id,
                    profile: source_profile,
                });
            }

            outgoing
                .entry(source_profile)
                .or_default()
                .push(step_id.clone());
            steps.insert(
                step_id,
                RegisteredStep {
                    implementation: edge.step,
                    descriptor,
                    direction: edge.direction,
                    verification: edge.verification,
                },
            );
        }

        for step_ids in outgoing.values_mut() {
            step_ids.sort();
        }

        Ok(Self {
            profiles: profile_map,
            steps,
            outgoing,
        })
    }

    /// Plans the unique shortest route over valid evidence-backed edges.
    pub fn plan(
        &self,
        source: &EffectiveProfile,
        target: &EffectiveProfile,
    ) -> Result<MigrationPlan, MigrationPlanError> {
        self.validate_endpoint(MigrationEndpoint::Source, source)?;
        self.validate_endpoint(MigrationEndpoint::Target, target)?;

        if source.id == target.id {
            return Ok(MigrationPlan {
                source_profile: source.id.clone(),
                target_profile: target.id.clone(),
                route_profiles: vec![source.id.clone()],
                step_ids: Vec::new(),
            });
        }

        let search = self.shortest_paths(&source.id, false);
        if let Some(distance) = search.distances.get(&target.id).copied() {
            if search.path_counts.get(&target.id).copied().unwrap_or(0) > 1 {
                return Err(MigrationPlanError::AmbiguousShortestPath {
                    source_profile: source.id.clone(),
                    target_profile: target.id.clone(),
                    steps: distance,
                });
            }
            return self.reconstruct_plan(source, target, &search);
        }

        let unrestricted = self.shortest_paths(&source.id, true);
        if unrestricted.distances.contains_key(&target.id) {
            let blocked_steps = self.blocked_edges_between(&source.id, &target.id);
            if !blocked_steps.is_empty() {
                return Err(MigrationPlanError::UnverifiedDowngradeRequired {
                    source_profile: source.id.clone(),
                    target_profile: target.id.clone(),
                    blocked_steps,
                });
            }
        }

        Err(MigrationPlanError::NoPath {
            source_profile: source.id.clone(),
            target_profile: target.id.clone(),
        })
    }

    fn validate_endpoint(
        &self,
        endpoint: MigrationEndpoint,
        profile: &EffectiveProfile,
    ) -> Result<(), MigrationPlanError> {
        let registered =
            self.profiles
                .get(&profile.id)
                .ok_or_else(|| MigrationPlanError::UnknownEndpoint {
                    endpoint,
                    profile: profile.id.clone(),
                })?;
        if registered != profile {
            return Err(MigrationPlanError::EndpointDefinitionMismatch {
                endpoint,
                profile: profile.id.clone(),
            });
        }
        Ok(())
    }

    fn shortest_paths(&self, source: &ProfileId, include_unverified: bool) -> PathSearch {
        let mut distances = BTreeMap::<ProfileId, usize>::new();
        let mut path_counts = BTreeMap::<ProfileId, u8>::new();
        let mut predecessors = BTreeMap::<ProfileId, (ProfileId, MigrationStepId)>::new();
        let mut queue = VecDeque::new();
        distances.insert(source.clone(), 0);
        path_counts.insert(source.clone(), 1);
        queue.push_back(source.clone());

        while let Some(profile) = queue.pop_front() {
            let Some(distance) = distances.get(&profile).copied() else {
                continue;
            };
            let next_distance = distance.saturating_add(1);
            let source_count = path_counts.get(&profile).copied().unwrap_or(0);
            for step_id in self.outgoing.get(&profile).into_iter().flatten() {
                let Some(step) = self.steps.get(step_id) else {
                    continue;
                };
                if !include_unverified && step.is_unverified_downgrade() {
                    continue;
                }
                let target = step.descriptor.target().exact_profile();
                match distances.get(target).copied() {
                    None => {
                        distances.insert(target.clone(), next_distance);
                        path_counts.insert(target.clone(), source_count.min(2));
                        predecessors.insert(target.clone(), (profile.clone(), step_id.clone()));
                        queue.push_back(target.clone());
                    }
                    Some(existing) if existing == next_distance => {
                        let current_count = path_counts.get(target).copied().unwrap_or(0);
                        path_counts.insert(
                            target.clone(),
                            current_count.saturating_add(source_count).min(2),
                        );
                        let candidate = (profile.clone(), step_id.clone());
                        if predecessors
                            .get(target)
                            .is_none_or(|current| candidate < *current)
                        {
                            predecessors.insert(target.clone(), candidate);
                        }
                    }
                    Some(_) => {}
                }
            }
        }

        PathSearch {
            distances,
            path_counts,
            predecessors,
        }
    }

    fn reconstruct_plan(
        &self,
        source: &EffectiveProfile,
        target: &EffectiveProfile,
        search: &PathSearch,
    ) -> Result<MigrationPlan, MigrationPlanError> {
        let mut reversed_steps = Vec::new();
        let mut reversed_profiles = vec![target.id.clone()];
        let mut current = target.id.clone();

        while current != source.id {
            let Some((previous, step_id)) = search.predecessors.get(&current) else {
                return Err(MigrationPlanError::NoPath {
                    source_profile: source.id.clone(),
                    target_profile: target.id.clone(),
                });
            };
            reversed_steps.push(step_id.clone());
            reversed_profiles.push(previous.clone());
            current = previous.clone();
        }

        reversed_steps.reverse();
        reversed_profiles.reverse();
        Ok(MigrationPlan {
            source_profile: source.id.clone(),
            target_profile: target.id.clone(),
            route_profiles: reversed_profiles,
            step_ids: reversed_steps,
        })
    }

    fn blocked_edges_between(
        &self,
        source: &ProfileId,
        target: &ProfileId,
    ) -> Vec<MigrationStepId> {
        let forward = self.reachable_profiles(source, false);
        let reverse = self.reverse_reachable_profiles(target);
        self.steps
            .iter()
            .filter(|(_, step)| {
                step.is_unverified_downgrade()
                    && forward.contains(step.descriptor.source().exact_profile())
                    && reverse.contains(step.descriptor.target().exact_profile())
            })
            .map(|(step_id, _)| step_id.clone())
            .collect()
    }

    fn reachable_profiles(&self, source: &ProfileId, verified_only: bool) -> BTreeSet<ProfileId> {
        let mut reached = BTreeSet::from([source.clone()]);
        let mut queue = VecDeque::from([source.clone()]);
        while let Some(profile) = queue.pop_front() {
            for step_id in self.outgoing.get(&profile).into_iter().flatten() {
                let Some(step) = self.steps.get(step_id) else {
                    continue;
                };
                if verified_only && step.is_unverified_downgrade() {
                    continue;
                }
                let target = step.descriptor.target().exact_profile().clone();
                if reached.insert(target.clone()) {
                    queue.push_back(target);
                }
            }
        }
        reached
    }

    fn reverse_reachable_profiles(&self, target: &ProfileId) -> BTreeSet<ProfileId> {
        let mut reverse = BTreeMap::<ProfileId, Vec<ProfileId>>::new();
        for step in self.steps.values() {
            reverse
                .entry(step.descriptor.target().exact_profile().clone())
                .or_default()
                .push(step.descriptor.source().exact_profile().clone());
        }
        let mut reached = BTreeSet::from([target.clone()]);
        let mut queue = VecDeque::from([target.clone()]);
        while let Some(profile) = queue.pop_front() {
            for predecessor in reverse.get(&profile).into_iter().flatten() {
                if reached.insert(predecessor.clone()) {
                    queue.push_back(predecessor.clone());
                }
            }
        }
        reached
    }

    pub(crate) fn profile(&self, id: &ProfileId) -> Option<&EffectiveProfile> {
        self.profiles.get(id)
    }

    pub(crate) fn registered_step(&self, id: &MigrationStepId) -> Option<&RegisteredStep> {
        self.steps.get(id)
    }
}

impl RegisteredStep {
    pub(crate) fn implementation(&self) -> &Arc<dyn MigrationStep> {
        &self.implementation
    }

    pub(crate) const fn descriptor(&self) -> &MigrationStepDescriptor {
        &self.descriptor
    }

    pub(crate) const fn is_unverified_downgrade(&self) -> bool {
        matches!(self.direction, MigrationDirection::Downgrade)
            && matches!(self.verification, MigrationVerification::Experimental)
    }
}

struct PathSearch {
    distances: BTreeMap<ProfileId, usize>,
    path_counts: BTreeMap<ProfileId, u8>,
    predecessors: BTreeMap<ProfileId, (ProfileId, MigrationStepId)>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::adapter::AdapterOutcome;
    use crate::artifact::ProfileId;
    use crate::diagnostic::{DiagnosticBuildError, DiagnosticReport};
    use crate::profile::{
        EffectiveProfile, ProfileRegistry, ProfileSourceKind, parse_profile_source,
        resolve_profiles,
    };

    use super::super::step::{
        MigrationAnalysis, MigrationAnalyzeRequest, MigrationApplyOutcome, MigrationApplyRequest,
        MigrationStepOutput, ProfileConstraint,
    };
    use super::*;

    struct TestStep {
        descriptor: MigrationStepDescriptor,
    }

    impl MigrationStep for TestStep {
        fn descriptor(&self) -> &MigrationStepDescriptor {
            &self.descriptor
        }

        fn analyze(
            &self,
            _request: MigrationAnalyzeRequest<'_>,
        ) -> Result<MigrationAnalysis, DiagnosticBuildError> {
            Ok(MigrationAnalysis::new(DiagnosticReport::new()))
        }

        fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
            AdapterOutcome::success(
                MigrationStepOutput::new(request.configuration().clone(), Vec::new()).unwrap(),
            )
        }
    }

    fn registry(ids: &[&str]) -> ProfileRegistry {
        let documents = ids.iter().map(|id| {
            parse_profile_source(
                &format!("{id}.json"),
                ProfileSourceKind::Bundled,
                &format!(r#"{{"schema_version":1,"id":"{id}","status":"experimental"}}"#),
            )
            .unwrap()
        });
        resolve_profiles(documents).unwrap()
    }

    fn profile(registry: &ProfileRegistry, id: &str) -> EffectiveProfile {
        registry
            .get(&ProfileId::parse(id).unwrap())
            .unwrap()
            .clone()
    }

    fn step(id: &str, source: &str, target: &str) -> Arc<dyn MigrationStep> {
        Arc::new(TestStep {
            descriptor: MigrationStepDescriptor::new(
                MigrationStepId::parse(id).unwrap(),
                ProfileConstraint::exact(ProfileId::parse(source).unwrap()),
                ProfileConstraint::exact(ProfileId::parse(target).unwrap()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        })
    }

    fn edge(id: &str, source: &str, target: &str) -> MigrationEdge {
        MigrationEdge::new(
            step(id, source, target),
            MigrationDirection::Lateral,
            MigrationVerification::Verified,
        )
    }

    fn route(graph: &MigrationGraph, registry: &ProfileRegistry) -> Vec<String> {
        graph
            .plan(
                &profile(registry, "profile:a"),
                &profile(registry, "profile:d"),
            )
            .unwrap()
            .step_ids()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn shuffled_registration_has_the_same_unique_shortest_route() {
        let profiles = registry(&["profile:a", "profile:b", "profile:c", "profile:d"]);
        let first = MigrationGraph::new(
            &profiles,
            vec![
                edge("migration:c-d", "profile:c", "profile:d"),
                edge("migration:a-b", "profile:a", "profile:b"),
                edge("migration:b-d", "profile:b", "profile:d"),
                edge("migration:b-c", "profile:b", "profile:c"),
            ],
        )
        .unwrap();
        let second = MigrationGraph::new(
            &profiles,
            vec![
                edge("migration:b-c", "profile:b", "profile:c"),
                edge("migration:b-d", "profile:b", "profile:d"),
                edge("migration:a-b", "profile:a", "profile:b"),
                edge("migration:c-d", "profile:c", "profile:d"),
            ],
        )
        .unwrap();

        assert_eq!(
            route(&first, &profiles),
            vec!["migration:a-b", "migration:b-d"]
        );
        assert_eq!(route(&first, &profiles), route(&second, &profiles));
    }

    #[test]
    fn no_path_is_typed_and_stable() {
        let profiles = registry(&["profile:a", "profile:b", "profile:d"]);
        let graph = MigrationGraph::new(
            &profiles,
            vec![edge("migration:a-b", "profile:a", "profile:b")],
        )
        .unwrap();
        let error = graph
            .plan(
                &profile(&profiles, "profile:a"),
                &profile(&profiles, "profile:d"),
            )
            .unwrap_err();
        assert!(matches!(error, MigrationPlanError::NoPath { .. }));
        assert_eq!(error.code(), "migration.plan-no-path");
    }

    #[test]
    fn equal_shortest_diamond_and_parallel_edges_are_ambiguous() {
        let diamond_profiles = registry(&["profile:a", "profile:b", "profile:c", "profile:d"]);
        let diamond = MigrationGraph::new(
            &diamond_profiles,
            vec![
                edge("migration:a-b", "profile:a", "profile:b"),
                edge("migration:b-d", "profile:b", "profile:d"),
                edge("migration:a-c", "profile:a", "profile:c"),
                edge("migration:c-d", "profile:c", "profile:d"),
            ],
        )
        .unwrap();
        assert!(matches!(
            diamond.plan(
                &profile(&diamond_profiles, "profile:a"),
                &profile(&diamond_profiles, "profile:d")
            ),
            Err(MigrationPlanError::AmbiguousShortestPath { steps: 2, .. })
        ));

        let parallel_profiles = registry(&["profile:a", "profile:d"]);
        let parallel = MigrationGraph::new(
            &parallel_profiles,
            vec![
                edge("migration:a-d-one", "profile:a", "profile:d"),
                edge("migration:a-d-two", "profile:a", "profile:d"),
            ],
        )
        .unwrap();
        assert!(matches!(
            parallel.plan(
                &profile(&parallel_profiles, "profile:a"),
                &profile(&parallel_profiles, "profile:d")
            ),
            Err(MigrationPlanError::AmbiguousShortestPath { steps: 1, .. })
        ));
    }

    #[test]
    fn self_loop_is_a_cycle_but_reverse_edges_are_valid() {
        let profiles = registry(&["profile:a", "profile:b"]);
        let self_loop = MigrationGraph::new(
            &profiles,
            vec![edge("migration:a-a", "profile:a", "profile:a")],
        );
        assert!(matches!(self_loop, Err(MigrationGraphError::Cycle { .. })));

        let reverse = MigrationGraph::new(
            &profiles,
            vec![
                edge("migration:a-b", "profile:a", "profile:b"),
                edge("migration:b-a", "profile:b", "profile:a"),
            ],
        )
        .unwrap();
        assert_eq!(
            reverse
                .plan(
                    &profile(&profiles, "profile:a"),
                    &profile(&profiles, "profile:b")
                )
                .unwrap()
                .step_ids(),
            [MigrationStepId::parse("migration:a-b").unwrap()]
        );
        assert!(
            reverse
                .plan(
                    &profile(&profiles, "profile:a"),
                    &profile(&profiles, "profile:a")
                )
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn duplicate_id_and_exact_registration_never_overwrite() {
        let profiles = registry(&["profile:a", "profile:b", "profile:c"]);
        let implementation = step("migration:a-b", "profile:a", "profile:b");
        let duplicate_edge = MigrationGraph::new(
            &profiles,
            vec![
                MigrationEdge::new(
                    Arc::clone(&implementation),
                    MigrationDirection::Lateral,
                    MigrationVerification::Verified,
                ),
                MigrationEdge::new(
                    implementation,
                    MigrationDirection::Lateral,
                    MigrationVerification::Verified,
                ),
            ],
        );
        assert!(matches!(
            duplicate_edge,
            Err(MigrationGraphError::DuplicateEdge { .. })
        ));

        let duplicate_id = MigrationGraph::new(
            &profiles,
            vec![
                edge("migration:duplicate", "profile:a", "profile:b"),
                edge("migration:duplicate", "profile:b", "profile:c"),
            ],
        );
        assert!(matches!(
            duplicate_id,
            Err(MigrationGraphError::DuplicateStepId { .. })
        ));
    }

    #[test]
    fn unverified_downgrade_is_never_traversed() {
        let profiles = registry(&["profile:new", "profile:old"]);
        let graph = MigrationGraph::new(
            &profiles,
            vec![MigrationEdge::new(
                step("migration:new-old", "profile:new", "profile:old"),
                MigrationDirection::Downgrade,
                MigrationVerification::Experimental,
            )],
        )
        .unwrap();
        let error = graph
            .plan(
                &profile(&profiles, "profile:new"),
                &profile(&profiles, "profile:old"),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            MigrationPlanError::UnverifiedDowngradeRequired { ref blocked_steps, .. }
                if blocked_steps == &[MigrationStepId::parse("migration:new-old").unwrap()]
        ));
        assert_eq!(error.code(), "migration.plan-unverified-downgrade");
    }

    #[test]
    fn verified_route_wins_over_a_shorter_unverified_downgrade() {
        let profiles = registry(&["profile:new", "profile:middle", "profile:old"]);
        let graph = MigrationGraph::new(
            &profiles,
            vec![
                MigrationEdge::new(
                    step("migration:new-old", "profile:new", "profile:old"),
                    MigrationDirection::Downgrade,
                    MigrationVerification::Experimental,
                ),
                MigrationEdge::new(
                    step("migration:new-middle", "profile:new", "profile:middle"),
                    MigrationDirection::Lateral,
                    MigrationVerification::Verified,
                ),
                MigrationEdge::new(
                    step("migration:middle-old", "profile:middle", "profile:old"),
                    MigrationDirection::Downgrade,
                    MigrationVerification::Verified,
                ),
            ],
        )
        .unwrap();

        let plan = graph
            .plan(
                &profile(&profiles, "profile:new"),
                &profile(&profiles, "profile:old"),
            )
            .unwrap();
        assert_eq!(
            plan.step_ids(),
            [
                MigrationStepId::parse("migration:new-middle").unwrap(),
                MigrationStepId::parse("migration:middle-old").unwrap(),
            ]
        );
    }

    #[test]
    fn graph_limit_is_checked_before_registration_processing() {
        let profiles = registry(&["profile:a", "profile:b"]);
        let repeated = edge("migration:a-b", "profile:a", "profile:b");
        let error = MigrationGraph::new(&profiles, vec![repeated; MAX_MIGRATION_GRAPH_STEPS + 1])
            .unwrap_err();
        assert!(matches!(
            error,
            MigrationGraphError::TooManySteps {
                maximum: MAX_MIGRATION_GRAPH_STEPS,
                actual,
            } if actual == MAX_MIGRATION_GRAPH_STEPS + 1
        ));
    }
}
