//! Local source-only PaletteColor contract witnessed in native BSP85 exports.
use crate::ir::{
    value::{PropertyValue, ValueKind},
    FieldId,
};
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
pub const F_SYNONYM: FieldId = FieldId(1);
pub const F_COMMENT: FieldId = FieldId(2);
pub const F_COLOR: FieldId = FieldId(5);
pub fn palette_color() -> &'static EntitySpec {
    static SPEC: std::sync::OnceLock<EntitySpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "PaletteColor",
        fields: Box::leak(
            vec![
                FieldSpec::with_default(
                    F_SYNONYM,
                    "synonym",
                    ValueKind::Localized,
                    PropertyValue::Localized(Vec::new()),
                )
                .normalized(Normalize::LocalizedSortByLang),
                FieldSpec::with_default(
                    F_COMMENT,
                    "comment",
                    ValueKind::Str,
                    PropertyValue::Str(String::new()),
                ),
                FieldSpec::required(F_COLOR, "color", ValueKind::Enum),
            ]
            .into_boxed_slice(),
        ),
        children: &[],
    })
}
