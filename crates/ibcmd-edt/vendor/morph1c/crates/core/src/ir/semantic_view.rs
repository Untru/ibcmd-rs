//! Explicit borrowed semantic serialization; ordinary IR wire serialization is unchanged.
use super::*;
use serde::ser::{SerializeSeq, SerializeStruct};

/// Recompute an optional semantic body from the current owner and current template.
pub type TemplateBodyProjection = fn(&MetadataObject, &Template) -> Result<Option<Vec<u8>>, String>;
/// Borrowed serialization view used only by source semantic comparison.
pub struct ConfigurationSemanticView<'a> {
    pub configuration: &'a Configuration,
    pub template_body: TemplateBodyProjection,
}
struct Objects<'a>(&'a [MetadataObject], TemplateBodyProjection);
struct Object<'a>(&'a MetadataObject, TemplateBodyProjection);
struct Templates<'a>(&'a MetadataObject, TemplateBodyProjection);
struct TemplateView<'a>(&'a MetadataObject, &'a Template, TemplateBodyProjection);
struct Properties<'a>(&'a [(FieldId, PropertyValue)]);
impl Serialize for Properties<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        super::serialize_semantic_properties(self.0, s)
    }
}
impl Serialize for Objects<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for obj in self.0 {
            seq.serialize_element(&Object(obj, self.1))?;
        }
        seq.end()
    }
}
impl Serialize for Templates<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.templates.len()))?;
        for template in &self.0.templates {
            seq.serialize_element(&TemplateView(self.0, template, self.1))?;
        }
        seq.end()
    }
}
impl Serialize for ConfigurationSemanticView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let obj = self.configuration;
        let mut state = s.serialize_struct("Configuration", 3)?;
        if obj.source_version.is_some() {
            state.serialize_field("source_version", &obj.source_version)?;
        }
        state.serialize_field("properties", &Properties(&obj.properties))?;
        state.serialize_field("objects", &Objects(&obj.objects, self.template_body))?;
        state.end()
    }
}
impl Serialize for Object<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let obj = self.0;
        let mut state = s.serialize_struct("MetadataObject", 30)?;
        state.serialize_field("kind", &obj.kind)?;
        state.serialize_field("name", &obj.name)?;
        state.serialize_field("uuid", &obj.uuid)?;
        if obj.internal_info.is_some() {
            state.serialize_field("internal_info", &obj.internal_info)?;
        }
        if obj.this_node.is_some() {
            state.serialize_field("this_node", &obj.this_node)?;
        }
        if obj.rights.is_some() {
            state.serialize_field("rights", &obj.rights)?;
        }
        if obj.xdto_schema.is_some() {
            state.serialize_field("xdto_schema", &obj.xdto_schema)?;
        }
        if obj.ws_definition.is_some() {
            state.serialize_field("ws_definition", &obj.ws_definition)?;
        }
        if obj.flowchart.is_some() {
            state.serialize_field("flowchart", &obj.flowchart)?;
        }
        if obj.command_interface.is_some() {
            state.serialize_field("command_interface", &obj.command_interface)?;
        }
        if obj.picture.is_some() {
            state.serialize_field("picture", &obj.picture)?;
        }
        if !obj.config_pictures.is_empty() {
            state.serialize_field("config_pictures", &obj.config_pictures)?;
        }
        if !obj.config_blobs.is_empty() {
            state.serialize_field("config_blobs", &obj.config_blobs)?;
        }
        if !obj.parent_configuration_resources.is_empty() {
            state.serialize_field(
                "parent_configuration_resources",
                &obj.parent_configuration_resources,
            )?;
        }
        if obj.standalone_content.is_some() {
            state.serialize_field("standalone_content", &obj.standalone_content)?;
        }
        if obj.root_command_interface.is_some() {
            state.serialize_field("root_command_interface", &obj.root_command_interface)?;
        }
        if obj.main_section_command_interface.is_some() {
            state.serialize_field(
                "main_section_command_interface",
                &obj.main_section_command_interface,
            )?;
        }
        if obj.home_page_work_area.is_some() {
            state.serialize_field("home_page_work_area", &obj.home_page_work_area)?;
        }
        if obj.client_application_interface.is_some() {
            state.serialize_field(
                "client_application_interface",
                &obj.client_application_interface,
            )?;
        }
        if !obj.help.is_empty() {
            state.serialize_field("help", &obj.help)?;
        }
        if !obj.help_resources.is_empty() {
            state.serialize_field("help_resources", &obj.help_resources)?;
        }
        if obj.schedule.is_some() {
            state.serialize_field("schedule", &obj.schedule)?;
        }
        if !obj.style_records.is_empty() {
            state.serialize_field("style_records", &obj.style_records)?;
        }
        state.serialize_field("properties", &Properties(&obj.properties))?;
        state.serialize_field("children", &Objects(&obj.children, self.1))?;
        state.serialize_field("modules", &obj.modules)?;
        state.serialize_field("forms", &obj.forms)?;
        if !obj.form_bodies.is_empty() {
            state.serialize_field("form_bodies", &obj.form_bodies)?;
        }
        state.serialize_field("templates", &Templates(obj, self.1))?;
        state.serialize_field("source_extensions", &obj.source_extensions)?;
        state.end()
    }
}
impl Serialize for TemplateView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let obj = self.1;
        let mut state = s.serialize_struct("Template", 5)?;
        state.serialize_field("name", &obj.name)?;
        state.serialize_field("properties", &Properties(&obj.properties))?;
        if obj.body.is_some() {
            let projected = (self.2)(self.0, obj).map_err(serde::ser::Error::custom)?;
            state.serialize_field("body", &projected.as_deref().or(obj.body.as_deref()))?;
        }
        if !obj.pages.is_empty() {
            state.serialize_field("pages", &obj.pages)?;
        }
        if !obj.resources.is_empty() {
            state.serialize_field("resources", &obj.resources)?;
        }
        state.end()
    }
}
