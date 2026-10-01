//! EDT reference pictures derive only their transparency BOOL from the referenced
//! CommonPicture's typed transparentPixel presence. Designer per-use BOOL/pixel
//! remain independent; no per-use pixel can be reconstructed from metadata.

use std::cell::RefCell;
use std::collections::BTreeMap;

use morph1c_core::ir::{DecoratorBody, DecoratorRef, FormBody, FormItem, PropertyValue};
use morph1c_core::spec::forms::command as fc;

use super::{FormError, fields, pictures, tables};

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
    visit_body(body, &mut resolve)?;
    body.common_picture_transparency = used;
    Ok(())
}

fn visit_body(
    body: &mut FormBody,
    visit: &mut impl FnMut(&mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for command in &mut body.commands {
        if let Some((_, picture)) = command
            .properties
            .iter_mut()
            .find(|(id, _)| *id == fc::F_PICTURE)
        {
            visit(picture)?;
        }
    }
    visit_items(&mut body.items, visit)?;
    if let Some(bar) = &mut body.auto_command_bar {
        visit_items(&mut bar.items, visit)?;
    }
    Ok(())
}

fn visit_items(
    items: &mut [FormItem],
    visit: &mut impl FnMut(&mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for item in items {
        for slot in pictures::picture_slots(&item.kind) {
            let bag = if slot.ext {
                &mut item.ext_info
            } else {
                &mut item.properties
            };
            if let Some((_, value)) = bag.iter_mut().find(|(id, _)| *id == slot.id) {
                visit(value)?;
            }
        }
        // ChoiceList's optional third item is itself a typed picture, not an
        // arbitrary nested list. Derive these slots from the same codec table.
        if let Some(kind) = tables::field_kind(item.kind.as_str()) {
            for projection in kind.ext {
                if matches!(projection.codec, fields::Codec::ChoiceList) {
                    if let Some((_, PropertyValue::List(choices))) = item
                        .ext_info
                        .iter_mut()
                        .find(|(id, _)| *id == projection.id)
                    {
                        for choice in choices {
                            if let PropertyValue::List(parts) = choice {
                                if let Some(picture) = parts.get_mut(2) {
                                    visit(picture)?;
                                }
                            }
                        }
                    }
                }
            }
        }
        visit_items(&mut item.children, visit)?;
        visit_items(&mut item.additions, visit)?;
        if let Some(table) = &mut item.auto_table {
            visit_items(std::slice::from_mut(table.as_mut()), visit)?;
        }
        if let Some(bar) = &mut item.auto_command_bar {
            visit_items(&mut bar.items, visit)?;
        }
        for decorator in [&mut item.context_menu, &mut item.ext_tooltip]
            .into_iter()
            .flatten()
        {
            visit_decorator(decorator, visit)?;
        }
    }
    Ok(())
}

fn visit_decorator(
    decorator: &mut DecoratorRef,
    visit: &mut impl FnMut(&mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    if let DecoratorBody::ContextMenu(menu) = &mut decorator.body {
        visit_items(&mut menu.items, visit)?;
    }
    Ok(())
}
