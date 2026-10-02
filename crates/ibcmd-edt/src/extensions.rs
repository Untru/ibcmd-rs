use crate::{EdtError, SourceExtensionUse};
use morph1c_core::ir::{Configuration, MetadataObject};

pub(crate) fn source_extensions_from_model(
    model: &Configuration,
) -> Result<Vec<SourceExtensionUse>, EdtError> {
    fn visit(
        objects: &[MetadataObject],
        resources: &mut usize,
        references: &mut usize,
    ) -> Result<(), EdtError> {
        for object in objects {
            for form in &object.form_bodies {
                if form.ordinary_body.is_none()
                    && let Some(count) =
                        formats_xml::form::picture_semantics_resource_count(&form.body)
                            .map_err(EdtError::source)?
                {
                    *resources = resources
                        .checked_add(1)
                        .ok_or_else(|| EdtError::new("extension resource count overflow"))?;
                    *references = references
                        .checked_add(count)
                        .ok_or_else(|| EdtError::new("extension reference count overflow"))?;
                }
            }
            visit(&object.children, resources, references)?;
        }
        Ok(())
    }
    let mut resources = 0;
    let mut references = 0;
    visit(&model.objects, &mut resources, &mut references)?;
    let mut extensions = if resources == 0 {
        Vec::new()
    } else {
        vec![SourceExtensionUse {
            id: "ibcmd-picture-semantics/1",
            resources,
            references,
        }]
    };
    fn event_visit(
        objects: &[MetadataObject],
        resources: &mut usize,
        references: &mut usize,
    ) -> Result<(), EdtError> {
        for object in objects {
            for form in &object.form_bodies {
                if form.ordinary_body.is_none()
                    && let Some(count) =
                        formats_xml::form::event_semantics_resource_count(&form.body)
                            .map_err(EdtError::source)?
                {
                    *resources = resources
                        .checked_add(1)
                        .ok_or_else(|| EdtError::new("event extension resource count overflow"))?;
                    *references = references
                        .checked_add(count)
                        .ok_or_else(|| EdtError::new("event extension reference count overflow"))?;
                }
            }
            event_visit(&object.children, resources, references)?;
        }
        Ok(())
    }
    let mut resources = 0;
    let mut references = 0;
    event_visit(&model.objects, &mut resources, &mut references)?;
    if resources > 0 {
        extensions.push(SourceExtensionUse {
            id: "ibcmd-form-event-semantics/1",
            resources,
            references,
        });
    }
    fn chart_visit(
        objects: &[MetadataObject],
        resources: &mut usize,
        references: &mut usize,
    ) -> Result<(), EdtError> {
        for object in objects {
            for form in &object.form_bodies {
                if form.ordinary_body.is_none()
                    && let Some(count) =
                        formats_xml::form::chart_semantics_resource_count(&form.body)
                            .map_err(EdtError::source)?
                {
                    *resources = resources
                        .checked_add(1)
                        .ok_or_else(|| EdtError::new("chart extension resource count overflow"))?;
                    *references = references
                        .checked_add(count)
                        .ok_or_else(|| EdtError::new("chart extension reference count overflow"))?;
                }
            }
            chart_visit(&object.children, resources, references)?;
        }
        Ok(())
    }
    let mut resources = 0;
    let mut references = 0;
    chart_visit(&model.objects, &mut resources, &mut references)?;
    if resources > 0 {
        extensions.push(SourceExtensionUse {
            id: "ibcmd-chart-semantics/1",
            resources,
            references,
        });
    }
    let defaults = formats_xml::metadata_picture_semantics::common_picture_defaults(&model.objects)
        .map_err(EdtError::new)?;
    fn metadata_visit(
        objects: &[MetadataObject],
        defaults: &std::collections::BTreeMap<String, bool>,
        resources: &mut usize,
        references: &mut usize,
    ) -> Result<(), EdtError> {
        for object in objects {
            if let Some(count) =
                formats_xml::metadata_picture_semantics::resource_count_with_defaults(
                    object, defaults,
                )
                .map_err(EdtError::new)?
            {
                *resources = resources
                    .checked_add(1)
                    .ok_or_else(|| EdtError::new("extension resource count overflow"))?;
                *references = references
                    .checked_add(count)
                    .ok_or_else(|| EdtError::new("extension reference count overflow"))?;
            }
            metadata_visit(&object.children, defaults, resources, references)?;
        }
        Ok(())
    }
    let mut resources = 0;
    let mut references = 0;
    metadata_visit(&model.objects, &defaults, &mut resources, &mut references)?;
    if resources > 0 {
        extensions.push(SourceExtensionUse {
            id: "ibcmd-metadata-picture-semantics/1",
            resources,
            references,
        });
    }
    for object in model
        .objects
        .iter()
        .filter(|o| o.kind.as_str() == "Configuration")
    {
        if let Some(references) =
            formats_xml::md_picture::resource_count(object).map_err(EdtError::new)?
        {
            extensions.push(SourceExtensionUse {
                id: "ibcmd-root-picture-semantics/1",
                resources: 1,
                references,
            });
        }
    }
    Ok(extensions)
}
