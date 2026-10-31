//! The restructuring engine at the root's `restructure::` paths, with the two
//! parts of the root's module that read the own apply's gate types: the S1
//! structural gate ([`s1`]) and the guard of its structure phase
//! ([`size_guard::guard_phase`]).

pub use ibcmd_restructure_engine::restructure::*;

pub mod s1;

/// The engine's size guard, and the guard of a structure phase the S1 gate
/// has prepared.
pub mod size_guard {
    pub use ibcmd_restructure_engine::restructure::size_guard::*;

    mod phase;
    pub use phase::guard_phase;
}

#[cfg(test)]
mod tests_plan;
#[cfg(test)]
mod tests_s1;
#[cfg(test)]
mod tests_sections;

#[cfg(test)]
#[path = "../../../../tests/support/restructure_plan.rs"]
mod test_plan_support;
#[cfg(test)]
#[path = "../../../../tests/support/restructure_sections.rs"]
#[expect(
    dead_code,
    reason = "Shared fixture helpers retain paths also consumed by the engine tests"
)]
mod test_sections_support;
