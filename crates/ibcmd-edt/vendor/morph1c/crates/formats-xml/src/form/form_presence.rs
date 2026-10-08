//! Closed CURRENT form-root presence transport. Native's implicit command
//! interface cannot reconstruct an independently authored presence boolean.
use super::{FormDialect, FormError, FormProjectionContext};
use morph1c_core::{
    ir::{FormBody, Uuid, form::FormRootPresence},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SCHEMA: &str = "urn:ibcmd:source-extension:form-presence:2";
pub const FORM_PRESENCE_RESOURCE: &str = "ibcmd-form-presence.v2.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resource {
    schema: String,
    version: u32,
    form_uuid: Uuid,
    profile: (u16, u16),
    root: FormRootPresence,
    projected_sha256: String,
}

/// One CURRENT invocation supplies native bytes and their exact root companion.
/// The companion must accompany these bytes when present.
#[derive(Debug, Clone)]
pub struct PreparedFormPresence {
    pub bytes: Vec<u8>,
    pub resource: Option<Vec<u8>>,
    pub chart_resource: Option<Vec<u8>>,
    pub picture_resource: Option<Vec<u8>>,
    pub event_resource: Option<Vec<u8>>,
    pub assets: Vec<(String, Vec<u8>)>,
}

/// A root record checked against the actual input artifact before other typed
/// resources restore paths or picture values in root-bar descendants.
pub struct FormPresenceRestoration {
    resource: Resource,
}

fn error(message: impl Into<String>) -> FormError {
    FormError::Frame(format!("form presence: {}", message.into()))
}
pub(super) fn validate_root(body: &FormBody) -> Result<(), FormError> {
    if !body.command_interface
        && (!body.form_ci_navigation_panel.is_empty() || !body.form_ci_command_bar.is_empty())
    {
        return Err(error(
            "command-interface panel values require their canonical command_interface presence",
        ));
    }
    Ok(())
}

fn root_digest(body: &FormBody) -> Result<String, FormError> {
    let bytes = serde_json::to_vec(&(
        &body.auto_command_bar,
        body.command_interface,
        &body.form_ci_navigation_panel,
        &body.form_ci_command_bar,
    ))
    .map_err(|e| error(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// This is the serializer's actual root convention, not an SDK normalization.
pub(super) fn needs_native_root_transport(body: &FormBody) -> bool {
    body.command_interface
        != (body.auto_command_bar.is_some()
            || !body.form_ci_navigation_panel.is_empty()
            || !body.form_ci_command_bar.is_empty())
}
pub fn form_presence_resource_count(body: &FormBody) -> Option<usize> {
    needs_native_root_transport(body).then_some(1)
}
pub fn same_form_presence_resource(a: &[u8], b: &[u8]) -> bool {
    match (parse_resource(a), parse_resource(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn native_forward(
    body: &FormBody,
    uuid: Uuid,
    profile: FormatVersion,
    context: Option<&FormProjectionContext<'_>>,
) -> Result<(Vec<u8>, FormBody, Option<Vec<u8>>), FormError> {
    let glyph_projection = super::project_native_picture_glyphs(body)?;
    let body = glyph_projection.as_ref().unwrap_or(body);
    let chart_projection = super::project_chart_semantics(body, uuid)?;
    let body = chart_projection.as_ref().map_or(body, |(body, _)| body);
    let bytes = with_roundtrip_target(profile, || {
        super::write::write_form_current(FormDialect::Designer, body, context)
    })?;
    let projected = with_source_version(Some(profile), || {
        super::read_form(FormDialect::Designer, &bytes)
    })?;
    Ok((bytes, projected, chart_projection.map(|(_, bytes)| bytes)))
}

/// Prepare the actual codec output and the closed CURRENT root restoration record
/// together, without mutating the caller's canonical body.
pub fn prepare_form_presence(
    body: &FormBody,
    uuid: Uuid,
    dialect: FormDialect,
    profile: FormatVersion,
    context: Option<&FormProjectionContext<'_>>,
) -> Result<PreparedFormPresence, FormError> {
    validate_root(body)?;
    with_roundtrip_target(profile, || {
        prepare_current(body, uuid, dialect, profile, context)
    })
}

fn prepare_current(
    body: &FormBody,
    uuid: Uuid,
    dialect: FormDialect,
    profile: FormatVersion,
    context: Option<&FormProjectionContext<'_>>,
) -> Result<PreparedFormPresence, FormError> {
    if dialect != FormDialect::Designer {
        return Ok(PreparedFormPresence {
            bytes: with_roundtrip_target(profile, || {
                super::write::write_form_current(dialect, body, context)
            })?,
            resource: None,
            chart_resource: None,
            picture_resource: None,
            event_resource: None,
            assets: Vec::new(),
        });
    }
    // Both native chart resources and root presence bind to this same CURRENT
    // snapshot and the same native glyph/chart projection invocation.
    let picture_resource = super::write_native_picture_resource(body, uuid)?;
    let event_resource = super::write_event_semantics_resource(body, uuid)?;
    let assets = super::choice_picture_assets(body, uuid)?;
    if !needs_native_root_transport(body) {
        let glyph_projection = super::project_native_picture_glyphs(body)?;
        let projected = glyph_projection.as_ref().unwrap_or(body);
        let chart_projection = super::project_chart_semantics(projected, uuid)?;
        let projected = chart_projection
            .as_ref()
            .map_or(projected, |(body, _)| body);
        return Ok(PreparedFormPresence {
            bytes: with_roundtrip_target(profile, || {
                super::write::write_form_current(dialect, projected, context)
            })?,
            resource: None,
            chart_resource: chart_projection.map(|(_, bytes)| bytes),
            picture_resource,
            event_resource,
            assets,
        });
    }
    let (bytes, projected, chart_resource) = native_forward(body, uuid, profile, context)?;
    if projected.command_interface == body.command_interface {
        return Err(error(
            "root convention disagrees with actual forward output",
        ));
    }
    let resource = Resource {
        schema: SCHEMA.into(),
        version: 2,
        form_uuid: uuid,
        profile: (profile.major, profile.minor),
        root: FormRootPresence {
            command_interface: body.command_interface,
        },
        projected_sha256: root_digest(&projected)?,
    };
    Ok(PreparedFormPresence {
        bytes,
        resource: Some(resource_bytes(&resource)?),
        chart_resource,
        picture_resource,
        event_resource,
        assets,
    })
}

pub(super) fn parse_resource(bytes: &[u8]) -> Result<Resource, FormError> {
    let resource: Resource =
        super::strict_resource::parse(bytes).map_err(|e| error(e.to_string()))?;
    if resource.schema != SCHEMA
        || resource.version != 2
        || resource.projected_sha256.len() != 64
        || !resource
            .projected_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(error("unknown schema/version or malformed root digest"));
    }
    Ok(resource)
}
pub(super) fn resource_uuid(resource: &Resource) -> Uuid {
    resource.form_uuid
}
pub(super) fn resource_profile(resource: &Resource) -> (u16, u16) {
    resource.profile
}
pub(super) fn resource_bytes(resource: &Resource) -> Result<Vec<u8>, FormError> {
    serde_json::to_vec(resource).map_err(|e| error(e.to_string()))
}

/// Restore an unpublished clone only after its actual forward root validates.
pub fn apply_form_presence_resource(
    body: &mut FormBody,
    uuid: Uuid,
    profile: FormatVersion,
    bytes: &[u8],
    context: Option<&FormProjectionContext<'_>>,
) -> Result<(), FormError> {
    validate_form_presence_resource(body, uuid, profile, bytes)?.restore(body, profile, context)
}

pub fn validate_form_presence_resource(
    body: &FormBody,
    uuid: Uuid,
    profile: FormatVersion,
    bytes: &[u8],
) -> Result<FormPresenceRestoration, FormError> {
    let resource = parse_resource(bytes)?;
    if resource.form_uuid != uuid || resource.profile != (profile.major, profile.minor) {
        return Err(error("foreign form or profile"));
    }
    if root_digest(body)? != resource.projected_sha256 {
        return Err(error(
            "current root/bar/panels differ from the bound forward output",
        ));
    }
    if body.command_interface == resource.root.command_interface {
        return Err(error("nonconsuming root facet"));
    }
    Ok(FormPresenceRestoration { resource })
}

impl FormPresenceRestoration {
    /// Called after all root-bar descendants' semantic resources are attached.
    /// Validation is complete before the caller's body receives the restored bool.
    pub fn restore(
        self,
        body: &mut FormBody,
        profile: FormatVersion,
        context: Option<&FormProjectionContext<'_>>,
    ) -> Result<(), FormError> {
        if self.resource.profile != (profile.major, profile.minor) {
            return Err(error("restoration profile changed"));
        }
        let mut restored = body.clone();
        restored.command_interface = self.resource.root.command_interface;
        verify(&restored, &self.resource, profile, context)?;
        *body = restored;
        Ok(())
    }
}

fn verify(
    body: &FormBody,
    resource: &Resource,
    profile: FormatVersion,
    context: Option<&FormProjectionContext<'_>>,
) -> Result<(), FormError> {
    validate_root(body)?;
    if body.command_interface != resource.root.command_interface
        || !needs_native_root_transport(body)
    {
        return Err(error("empty or nonconsuming root facet"));
    }
    let (_, projected, _) = with_roundtrip_target(profile, || {
        native_forward(body, resource.form_uuid, profile, context)
    })?;
    if root_digest(&projected)? != resource.projected_sha256 {
        return Err(error(
            "CURRENT root does not reproduce its bound forward output",
        ));
    }
    Ok(())
}

pub(super) fn verify_restored(
    body: &FormBody,
    uuid: Uuid,
    profile: FormatVersion,
    resource: &Resource,
    context: &FormProjectionContext<'_>,
) -> Result<(), FormError> {
    if resource.form_uuid != uuid || resource.profile != (profile.major, profile.minor) {
        return Err(error("restored owner/profile differs"));
    }
    verify(body, resource, profile, Some(context))
}
