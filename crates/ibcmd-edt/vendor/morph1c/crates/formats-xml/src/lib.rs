//! `formats-xml` — общий XML-проекционный субстрат для XML-семейства форматов 1С
//! (EDT `.mdo`, Designer `.xml`). XML — формат-слой, НЕ `core` (ARCHITECTURE.md
//! §1.4/§5): и `edt`, и `designer` зависят отсюда.
//!
//! Что даёт крейт:
//! * [`XmlLocus`]/[`Codec`] — расширяемая модель «где лежит и как кодируется поле».
//! * [`read::parse`] — токенизация дескриптора в дерево [`descriptor::Element`] с
//!   полным учётом узлов (граница тотальности §1.0).
//! * [`emit`] — byte-exact структурный эмиттер (восстанавливает дескриптор из
//!   IR+спека+envelope, НЕ эхает вход).
//! * [`XmlProjection`] — реализация [`morph1c_core::Projection`], драйвящая
//!   `core::engine::{read,write}` для XML по карте `FieldId → (XmlLocus, Codec)`.
//!
//! Канонический `id`/`value_kind`/порядок/нормализация/омиссия дефолтов — в
//! `core/spec` и движке; здесь — только имена тегов/ns и байтовая обёртка (§1.6).

pub mod characteristics;
pub mod children;
pub mod choice_param_links;
pub mod choice_parameters;
pub mod common_attribute_content;
pub mod configuration;
pub mod derive;
pub mod descriptor;
pub mod emit;
pub mod exchange_plan_content;
pub mod form;
pub mod form_tree;
pub mod geoschema;
pub mod link_by_type;
pub mod locus;
pub mod picture;
pub mod picture_sidecar;
pub mod predefined;
pub mod predefined_cct;
pub mod predefined_coa;
pub mod produced_types;
pub mod read;
pub mod ref_list;
pub mod registry;
pub mod rights;
pub mod shortcut;
pub mod source_extensions;
pub mod std_attrs_generic;
pub mod std_attrs_ir;
pub mod std_tabular_sections;
pub mod style_value_codec;
pub mod transparent_pixel;
pub mod type_codec;
pub mod value_codec;
pub mod xdto;
pub mod xdto_packages;
pub mod xdto_type_ref;

mod codec;
mod compat_mode;
mod projection;

#[cfg(any())]
mod tests;

pub use descriptor::{Attr, Element};
pub use emit::{escape_attr, escape_text, Envelope, OutElement};
pub use locus::{ChildLocus, Codec, FieldProjection, StdAttrsDialect, XmlLocus};
pub use read::{parse, ByteEnvelope, Descriptor, EolStyle, XmlReadError};
pub use registry::{CorpusLayout, Format, FormatKind, SidecarFormat};
pub use style_value_codec::StyleValueDialect;
pub use type_codec::TypeDialect;
pub use value_codec::ValueDialect;
pub use xdto_packages::XdtoPackagesDialect;
pub use xdto_type_ref::XdtoTypeRefDialect;
pub use projection::{claim_props, LocusMap, XmlProjection, XmlSink, XmlSource};
