//! EDT reference pictures derive only their transparency BOOL from the referenced
//! CommonPicture's typed transparentPixel presence. Designer per-use BOOL/pixel
//! remain independent; no per-use pixel can be reconstructed from metadata.

use std::cell::RefCell;
use std::collections::BTreeMap;

use morph1c_core::ir::{FormBody, PropertyValue};

use super::{FormError, fields};

thread_local! {
    static DEFAULTS: RefCell<BTreeMap<String, bool>> = const { RefCell::new(BTreeMap::new()) };
}

pub(crate) fn common_picture_default(reference: &str) -> Option<bool> {
    DEFAULTS.with(|defaults| defaults.borrow().get(reference).copied())
}

pub(crate) fn with_common_picture_defaults<T>(
    defaults: &BTreeMap<String, bool>,
    f: impl FnOnce() -> T,
) -> T {
    struct Restore(BTreeMap<String, bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DEFAULTS.with(|defaults| *defaults.borrow_mut() = std::mem::take(&mut self.0));
        }
    }
    let _restore = Restore(DEFAULTS.with(|slot| slot.replace(defaults.clone())));
    f()
}

/// Bind typed CommonPicture metadata to a managed form. For EDT-origin bodies
/// only the per-use BOOL is projected; Designer-origin BOOL/pixel stay intact.
/// Missing referenced metadata rejects instead of guessing a transparency flag.
pub fn resolve_common_picture_transparency(
    body: &mut FormBody,
    metadata: &BTreeMap<String, bool>,
    edt_origin: bool,
) -> Result<(), FormError> {
    let mut used = BTreeMap::new();
    let mut resolve = |value: &mut PropertyValue| -> Result<(), FormError> {
        let (reference, _) = fields::picture_ref_lt(value)?;
        if !reference.starts_with("CommonPicture.") {
            return Ok(());
        }
        let reference = reference.to_owned();
        let flag = *metadata.get(&reference).ok_or_else(|| {
            FormError::Frame(format!(
                "unresolved typed CommonPicture reference {reference:?}"
            ))
        })?;
        used.insert(reference.clone(), flag);
        if edt_origin {
            if fields::picture_pixel(value).is_some() {
                return Err(FormError::Frame(
                    "EDT CommonPicture reference cannot carry an independent per-use pixel".into(),
                ));
            }
            *value = fields::picture_canon(reference, flag);
        }
        Ok(())
    };
    super::picture_semantics::visit(body, &mut |_, value| resolve(value))?;
    body.common_picture_transparency = used;
    Ok(())
}
