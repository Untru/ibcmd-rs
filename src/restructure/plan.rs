//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::plan::{
    Addition, AlterColumn, CacheUpdate, CreatedSection, IndexSwitch, Inputs, Method, ObjectPlan,
    PROMOTE_CONFIG, Phase, Plan, PlanOptions, Removal, SectionPlan, StagedImage, Statement,
    StatementParam, TablePlan, Widening, configuration_uuid, parse_deleted, plan,
};

#[expect(
    unused_imports,
    reason = "Retain the original crate-internal compatibility paths"
)]
pub(crate) use ibcmd_restructure_engine::restructure::plan::{Running, indexing_word, row_bytes};
