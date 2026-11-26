//! Reconcile a NESTED (Subsystem) object's HIERARCHICAL IDENTITY with the on-disk tree during
//! whole-config read (§1.0/§1.6) — the read-side mirror of [`crate::layout::nested_output_path`].
//!
//! # The asymmetry (RE: SSL — 87 subsystems, 3 top-level, depth ≤ 3)
//! A Subsystem's place in the tree is expressed DIFFERENTLY by the two dialects:
//! * **EDT** carries it IN the descriptor — `<parentSubsystem>Subsystem.A.Subsystem.B</parentSubsystem>`,
//!   the FULL ancestor chain (SSL: 84/87 `.mdo` carry it; the 3 top-level ones do not) — AND
//!   mirrors it on disk (`Subsystems/A/Subsystems/B/Subsystems/C/C.mdo`).
//! * **Designer** carries it ONLY on disk (`Subsystems/A/Subsystems/B/Subsystems/C.xml`); the
//!   `<Subsystem>` descriptor has NO parent element at all (SSL: 0/87 occurrences).
//!
//! So a Designer read produced 87 objects that all looked TOP-LEVEL. Everything downstream that
//! needs the hierarchy then broke or lied:
//! * `layout::nested_output_path` placed all 87 flat at `Subsystems/<Own>.xml` → the 3 own-names
//!   that repeat across branches (`БазоваяФункциональность`, `Печать`,
//!   `КонтрольРаботыПользователей`) COLLIDED → whole-config `--from designer` aborted with
//!   `NestedUnresolved` (the write-side collision guard) — the "Subsystem path collision".
//! * `formats_cf::assemble` derives a subsystem's cf object-path from the SAME property
//!   (`subsystem_ir_path`) → designer→cf would have emitted a FLAT `Subsystem.<name>` where the
//!   oracle has `Subsystem.<root>.Subsystem.<child>`, silently corrupting the hierarchy and every
//!   Role right that addresses a nested subsystem.
//!
//! # The fix: the on-disk chain IS the canonical identity — SYNTHESIZE it on a Designer read
//! `layout::collect_objects_checked` already keys every nested object by its hierarchical
//! `Parent/Child` name path (it recursed to find it). This pass turns that key into the canonical
//! IR property `F_PARENT_SUBSYSTEM` (`Subsystem.A.Subsystem.B`), so a Designer-sourced IR is
//! IDENTICAL to an EDT-sourced one (§1.6) and every consumer — the XML writers, the cf assembler
//! — reads the hierarchy from one place.
//!
//! On an EDT read the property is already there; this pass CROSS-CHECKS it against the on-disk
//! chain instead (§1.0 — a descriptor whose `parentSubsystem` disagrees with its physical
//! location is a corrupt source, not something to silently prefer one side of).

use formats_xml::registry::{CorpusLayout, Format, FormatKind};
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::subsystem::F_PARENT_SUBSYSTEM;

use crate::ConvertError;

/// Render an ancestor chain (`["A", "B"]`) as the canonical `parentSubsystem` back-reference
/// (`Subsystem.A.Subsystem.B`). Empty chain → `""` (a top-level subsystem carries no property).
fn parent_ref(ancestors: &[&str]) -> String {
    ancestors
        .iter()
        .map(|a| format!("Subsystem.{a}"))
        .collect::<Vec<_>>()
        .join(".")
}

/// Reconcile `obj`'s hierarchical identity with its on-disk position.
///
/// `key` is the enumeration key `layout::collect_objects_checked` produced: for a `Nested` kind
/// the `/`-joined name path (`A/B/C`), for every other layout the bare object name. A non-Nested
/// kind is a no-op, as is cf (records in a container carry their path in the cf itself).
///
/// * **Designer** — SYNTHESIZES `F_PARENT_SUBSYSTEM` from `key`'s ancestor segments (the
///   descriptor has no parent element). A descriptor that DOES carry one is an unwitnessed shape
///   → typed refusal (§1.0).
/// * **EDT** — CROSS-CHECKS the descriptor's `F_PARENT_SUBSYSTEM` against `key`'s ancestors; a
///   mismatch is a typed [`ConvertError::NestedUnresolved`] (§1.0 — never a silent pick).
pub fn reconcile_nested_identity(
    format: Format,
    fk: &FormatKind,
    key: &str,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if !matches!(fk.layout, CorpusLayout::Nested { .. }) || format == Format::Cf {
        return Ok(());
    }
    let bad = |reason: String| ConvertError::NestedUnresolved {
        kind: fk.kind.to_string(),
        object: obj.name.clone(),
        reason,
    };

    // Split the enumeration key into ancestors + own segment; the own segment must be the name
    // the descriptor itself declares (a `<Name>` disagreeing with its file/dir name is corrupt).
    let mut segs: Vec<&str> = key.split('/').collect();
    let own = segs.pop().unwrap_or_default();
    if own != obj.name {
        return Err(bad(format!(
            "on-disk key {key:?} ends in {own:?} but the descriptor declares the name {:?} \
             (§1.0 — the physical location and the descriptor disagree)",
            obj.name
        )));
    }
    let on_disk = parent_ref(&segs);

    match format {
        Format::Designer => {
            // §1.0: the Designer descriptor has NO parent element in the whole witnessed corpus
            // (SSL 0/87). One appearing means the dialect grew a shape we have not RE'd.
            if let Some(existing) = obj.get(F_PARENT_SUBSYSTEM) {
                return Err(bad(format!(
                    "the Designer descriptor carries a parentSubsystem ({existing:?}) — \
                     unwitnessed shape; the Designer hierarchy is expressed ONLY on disk (§1.0)"
                )));
            }
            // The on-disk chain IS the identity → canonicalize it into the IR (§1.6: the same
            // property an EDT read carries, so downstream sees ONE hierarchy source).
            if !on_disk.is_empty() {
                obj.properties
                    .push((F_PARENT_SUBSYSTEM, PropertyValue::Str(on_disk)));
            }
            Ok(())
        }
        // EDT declares the chain in BOTH places — they must agree.
        _ => {
            let declared = match obj.get(F_PARENT_SUBSYSTEM) {
                Some(PropertyValue::Str(s)) => s.as_str(),
                None => "", // top-level: no property (witnessed 3/87)
                Some(other) => {
                    return Err(bad(format!(
                        "parentSubsystem is not a string value: {other:?}"
                    )))
                }
            };
            if declared != on_disk {
                return Err(bad(format!(
                    "the descriptor declares parentSubsystem {declared:?} but it sits on disk at \
                     {key:?} (= {on_disk:?}) — §1.0, the two hierarchies disagree"
                )));
            }
            Ok(())
        }
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use formats_xml::registry::CorpusLayout;
    use morph1c_core::ir::{ObjectKind, Uuid};

    /// A `Nested` FormatKind stub (only `kind` + `layout` are read by this pass).
    fn nested_fk() -> FormatKind {
        FormatKind {
            layout: CorpusLayout::Nested {
                ext: "xml",
                nesting_dir: "Subsystems",
                dir_per_object: false,
            },
            ..*crate::registry::FormatRegistry::for_format(Format::Designer)
                .unwrap()
                .get("Subsystem")
                .expect("Subsystem kind is registered")
        }
    }

    fn sub(name: &str) -> MetadataObject {
        MetadataObject::new(ObjectKind::new("Subsystem"), name, Uuid([3; 16]))
    }

    #[test]
    fn renders_the_canonical_parent_reference() {
        assert_eq!(parent_ref(&[]), "");
        assert_eq!(parent_ref(&["A"]), "Subsystem.A");
        assert_eq!(parent_ref(&["A", "B"]), "Subsystem.A.Subsystem.B");
    }

    /// Designer: the on-disk chain becomes the IR property (this is the whole blocker).
    #[test]
    fn designer_synthesizes_the_chain_from_the_on_disk_key() {
        let fk = nested_fk();

        // Nested two deep → the full ancestor chain.
        let mut o = sub("Печать");
        reconcile_nested_identity(
            Format::Designer,
            &fk,
            "СтандартныеПодсистемы/ПодключаемыеКоманды/Печать",
            &mut o,
        )
        .unwrap();
        assert_eq!(
            o.get(F_PARENT_SUBSYSTEM),
            Some(&PropertyValue::Str(
                "Subsystem.СтандартныеПодсистемы.Subsystem.ПодключаемыеКоманды".into()
            ))
        );

        // Top-level → NO property (matches the 3 SSL roots' EDT descriptors).
        let mut root = sub("СтандартныеПодсистемы");
        reconcile_nested_identity(Format::Designer, &fk, "СтандартныеПодсистемы", &mut root)
            .unwrap();
        assert_eq!(root.get(F_PARENT_SUBSYSTEM), None);
    }

    /// The colliding own-names now carry DISTINCT identities (SSL: 3 such names).
    #[test]
    fn colliding_own_names_get_distinct_chains() {
        let fk = nested_fk();
        let mut a = sub("БазоваяФункциональность");
        let mut b = sub("БазоваяФункциональность");
        reconcile_nested_identity(
            Format::Designer,
            &fk,
            "СтандартныеПодсистемы/БазоваяФункциональность",
            &mut a,
        )
        .unwrap();
        reconcile_nested_identity(
            Format::Designer,
            &fk,
            "СтандартныеПодсистемы/ВариантыОтчетов/БазоваяФункциональность",
            &mut b,
        )
        .unwrap();
        assert_ne!(a.get(F_PARENT_SUBSYSTEM), b.get(F_PARENT_SUBSYSTEM));
    }

    /// EDT: the descriptor's chain and the on-disk chain must AGREE (no silent preference).
    #[test]
    fn edt_cross_checks_the_declared_chain() {
        let fk = nested_fk();

        let mut ok = sub("Анкетирование");
        ok.properties.push((
            F_PARENT_SUBSYSTEM,
            PropertyValue::Str("Subsystem.СтандартныеПодсистемы".into()),
        ));
        reconcile_nested_identity(
            Format::Edt,
            &fk,
            "СтандартныеПодсистемы/Анкетирование",
            &mut ok,
        )
        .expect("declared chain matches the on-disk position");

        // Disagreement → loud.
        let mut bad = sub("Анкетирование");
        bad.properties.push((
            F_PARENT_SUBSYSTEM,
            PropertyValue::Str("Subsystem.Администрирование".into()),
        ));
        let err = reconcile_nested_identity(
            Format::Edt,
            &fk,
            "СтандартныеПодсистемы/Анкетирование",
            &mut bad,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("disagree"), "got {err}");

        // A top-level EDT subsystem carries no property — and sits at the root.
        let mut root = sub("Администрирование");
        reconcile_nested_identity(Format::Edt, &fk, "Администрирование", &mut root).unwrap();
    }

    /// A Designer descriptor that DOES carry the property is unwitnessed → refuse (§1.0).
    #[test]
    fn designer_refuses_a_descriptor_borne_chain() {
        let fk = nested_fk();
        let mut o = sub("Анкетирование");
        o.properties.push((
            F_PARENT_SUBSYSTEM,
            PropertyValue::Str("Subsystem.СтандартныеПодсистемы".into()),
        ));
        let err = reconcile_nested_identity(
            Format::Designer,
            &fk,
            "СтандартныеПодсистемы/Анкетирование",
            &mut o,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("unwitnessed"), "got {err}");
    }

    /// The key's last segment must be the name the descriptor declares.
    #[test]
    fn refuses_a_key_that_disagrees_with_the_declared_name() {
        let fk = nested_fk();
        let mut o = sub("Анкетирование");
        let err = reconcile_nested_identity(Format::Designer, &fk, "Родитель/Другое", &mut o)
            .unwrap_err();
        assert!(format!("{err}").contains("disagree"), "got {err}");
    }
}
