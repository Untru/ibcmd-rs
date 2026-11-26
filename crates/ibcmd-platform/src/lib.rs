//! The portable platform registry: every 1C:Enterprise build ibcmd-rs knows,
//! and what its bundled profile declares for the XML it reads and writes and
//! for the rows it stores.
//!
//! The builds and what they declare live in `profiles/platform/*.json`, the
//! bundled profiles `ibcmd-compiler-families` compiles in. This crate reads
//! their constants -- `platform.xml_format`, `platform.form_layout`,
//! `platform.feature.<name>` and `platform.form.default_button_importance` --
//! instead of repeating them in code, and answers from them alone:
//!
//! - a build's XML format ([`Platform::xml_format`]) and stored form layout
//!   ([`Platform::form_layout`]);
//! - the features a configuration saved by it may list in its `version` row
//!   ([`Platform::features`], [`feature_uuid`]);
//! - its release and packed compatibility ([`Platform::release`],
//!   [`Platform::compatibility_packed`]);
//! - the release that reads and writes an XML dialect ([`of_xml_dialect`]).
//!
//! A build whose profile declares no XML format (the 8.3.24.1819 seed) is
//! known but not supported. A platform is named by its full version: an exact
//! build (`8.3.27.2214`) or a release (`8.3.27`), which stands for every known
//! build of it and exists only while they agree on what they declare. A
//! version the registry does not know is refused with the list of known ones,
//! never mapped to the nearest one.
//!
//! The root's `platform` module wraps each [`Platform`] in its `PlatformSpec`,
//! adding what only the root knows: the command line's XML selector and the
//! MSSQL capabilities of a build. Naming rules and what a new build needs are
//! in that module's documentation.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_core::profile::EffectiveProfile;
use serde::{Serialize, Serializer};

/// Profile constant: the XML format a build reads and writes (`2.20`, `2.21`).
pub const CONSTANT_XML_FORMAT: &str = "platform.xml_format";
/// Profile constant: the stored form layout of a build (`8.3`, `8.5.1`).
pub const CONSTANT_FORM_LAYOUT: &str = "platform.form_layout";
/// Profile constant prefix: `platform.feature.<name>` = the feature's uuid.
pub const CONSTANT_FEATURE_PREFIX: &str = "platform.feature.";
/// Profile constant: `main` when the build writes `ButtonImportance` `Main`
/// on a default button whose record holds the code of no importance of its
/// own (8.5.1.1529; 8.5.1.1150 writes nothing there).
pub const CONSTANT_DEFAULT_BUTTON_IMPORTANCE: &str = "platform.form.default_button_importance";
/// The palette-colours feature (since 8.5.1): every `PaletteColor` object a
/// configuration holds reports it, and its `version` row lists it.
pub const FEATURE_PALETTE_COLORS: &str = "palette-colors";

// The stored layout of managed-form bodies is the `ibcmd-form-body` crate's,
// with the 8.5 layout's conversion.
pub use ibcmd_form_body::FormLayout;

/// The layout a profile's `platform.form_layout` constant names.
fn parse_form_layout(text: &str) -> Option<FormLayout> {
    match text {
        "8.3" => Some(FormLayout::V8_3),
        "8.5.1" => Some(FormLayout::V8_5_1),
        _ => None,
    }
}

/// An XML format a known build reads and writes, as its profile's
/// `platform.xml_format` declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XmlFormat {
    /// XML 2.20, read and written by 8.3.x.
    V2_20,
    /// XML 2.21, read and written by 8.5.x.
    V2_21,
}

impl XmlFormat {
    /// The format as profiles and dialect names spell it: `2.20`, `2.21`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V2_20 => "2.20",
            Self::V2_21 => "2.21",
        }
    }
}

/// A feature a configuration saved by the platform can list in its
/// `version` row, which an older platform that does not know it refuses.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PlatformFeature {
    name: String,
    uuid: String,
}

impl PlatformFeature {
    /// The feature's name in the profiles (`palette-colors`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The fixed uuid the platform's feature registry gives it.
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
}

/// One known build, or one release standing for its builds.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    /// `8.3.27.2214` or `8.3.27`.
    name: String,
    release: [u32; 3],
    /// The fourth component of an exact build.
    build: Option<u32>,
    xml: XmlFormat,
    form_layout: FormLayout,
    features: Vec<PlatformFeature>,
    /// The bundled profile of an exact build; a release has none.
    profile: Option<EffectiveProfile>,
    /// See [`CONSTANT_DEFAULT_BUTTON_IMPORTANCE`]. A release never has it: it
    /// stands for the builds that agree on the XML format, the form layout and
    /// the features, and keeps the behavior of the oldest of them.
    default_button_importance: bool,
}

#[derive(Debug)]
struct Registry {
    /// Exact builds, then releases, each in version order.
    entries: Vec<Entry>,
    /// Builds a profile names without declaring an XML format.
    unsupported: Vec<String>,
    /// Releases whose builds disagree, so only a build names them.
    ambiguous_releases: Vec<String>,
}

static REGISTRY: OnceLock<std::result::Result<Registry, String>> = OnceLock::new();

fn registry() -> Result<&'static Registry> {
    REGISTRY
        .get_or_init(|| build_registry().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow!("the bundled platform registry is invalid: {error}"))
}

fn build_registry() -> Result<Registry> {
    let profiles = ibcmd_compiler_families::profile_registry::load_bundled_profile_registry()?;
    let mut builds = Vec::new();
    let mut unsupported = Vec::new();
    for profile in profiles.profiles().values() {
        let Some(build) = &profile.platform_build else {
            continue;
        };
        let name = build.value.to_string();
        let components = build.value.as_version().components();
        let &[major, minor, patch, number] = components else {
            bail!(
                "platform profile `{}` names `{name}`, not a four-part build",
                profile.id
            );
        };
        match build_entry(profile, name.clone(), [major, minor, patch], number)? {
            Some(entry) => builds.push(entry),
            None => unsupported.push(name),
        }
    }
    builds.sort_by_key(|entry| (entry.release, entry.build));

    let mut by_release = BTreeMap::<[u32; 3], Vec<&Entry>>::new();
    for entry in &builds {
        by_release.entry(entry.release).or_default().push(entry);
    }
    let mut releases = Vec::new();
    let mut ambiguous_releases = Vec::new();
    for (release, members) in by_release {
        let first = members[0];
        let agree = members.iter().all(|entry| {
            entry.xml == first.xml
                && entry.form_layout == first.form_layout
                && entry.features == first.features
        });
        let name = release_name(release);
        if !agree {
            ambiguous_releases.push(name);
            continue;
        }
        releases.push(Entry {
            name,
            release,
            build: None,
            xml: first.xml,
            form_layout: first.form_layout,
            features: first.features.clone(),
            profile: None,
            default_button_importance: false,
        });
    }
    let mut entries = builds;
    entries.extend(releases);
    Ok(Registry {
        entries,
        unsupported,
        ambiguous_releases,
    })
}

/// A build entry from its profile, or `None` when the profile declares no
/// XML format (a build ibcmd-rs does not read or write).
fn build_entry(
    profile: &EffectiveProfile,
    name: String,
    release: [u32; 3],
    build: u32,
) -> Result<Option<Entry>> {
    let constant = |key: &str| profile.constants.get(key).map(|value| value.value.as_str());
    let Some(xml) = constant(CONSTANT_XML_FORMAT) else {
        return Ok(None);
    };
    let xml = match xml {
        "2.20" => XmlFormat::V2_20,
        "2.21" => XmlFormat::V2_21,
        other => bail!(
            "platform profile `{}` declares {CONSTANT_XML_FORMAT} `{other}`, which ibcmd-rs does not read or write",
            profile.id
        ),
    };
    let layout = constant(CONSTANT_FORM_LAYOUT).ok_or_else(|| {
        anyhow!(
            "platform profile `{}` declares its XML format but no {CONSTANT_FORM_LAYOUT}",
            profile.id
        )
    })?;
    let form_layout = parse_form_layout(layout).ok_or_else(|| {
        anyhow!(
            "platform profile `{}` declares an unknown {CONSTANT_FORM_LAYOUT} `{layout}`",
            profile.id
        )
    })?;
    let mut features = Vec::new();
    for (key, value) in &profile.constants {
        let Some(feature) = key.strip_prefix(CONSTANT_FEATURE_PREFIX) else {
            continue;
        };
        let uuid = uuid::Uuid::parse_str(&value.value).with_context(|| {
            format!(
                "platform profile `{}` declares feature `{feature}` with a malformed uuid",
                profile.id
            )
        })?;
        features.push(PlatformFeature {
            name: feature.to_string(),
            uuid: uuid.hyphenated().to_string(),
        });
    }
    features.sort();
    let default_button_importance = match constant(CONSTANT_DEFAULT_BUTTON_IMPORTANCE) {
        None => false,
        Some("main") => true,
        Some(other) => bail!(
            "platform profile `{}` declares an unknown {CONSTANT_DEFAULT_BUTTON_IMPORTANCE} `{other}`",
            profile.id
        ),
    };
    Ok(Some(Entry {
        name,
        release,
        build: Some(build),
        xml,
        form_layout,
        features,
        profile: Some(profile.clone()),
        default_button_importance,
    }))
}

/// `8.3.27` for `[8, 3, 27]`.
pub fn release_name(release: [u32; 3]) -> String {
    format!("{}.{}.{}", release[0], release[1], release[2])
}

/// A known platform: an exact build or a release.
///
/// Cheap to copy; two platforms are equal when they name the same entry.
#[derive(Clone, Copy)]
pub struct Platform {
    entry: &'static Entry,
}

impl PartialEq for Platform {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.entry, other.entry)
    }
}

impl Eq for Platform {}

impl fmt::Debug for Platform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Platform")
            .field(&self.entry.name)
            .finish()
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.entry.name)
    }
}

impl Serialize for Platform {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.entry.name)
    }
}

impl Platform {
    /// The XML format the platform reads and writes: 2.20 for 8.3.x, 2.21 for
    /// 8.5.x.
    pub fn xml_format(&self) -> XmlFormat {
        self.entry.xml
    }

    /// The platform's own stored layout of forms, colours and fonts; a
    /// configuration kept in an older compatibility mode may store the
    /// previous one ([`FormLayout::stored`]).
    pub fn form_layout(&self) -> FormLayout {
        self.entry.form_layout
    }

    /// The features beyond older platforms' that the platform knows, by name.
    pub fn features(&self) -> &'static [PlatformFeature] {
        &self.entry.features
    }

    /// One feature by name, when the platform knows it.
    pub fn feature(&self, name: &str) -> Option<&'static PlatformFeature> {
        self.entry
            .features
            .iter()
            .find(|feature| feature.name == name)
    }

    /// The version as given: `8.5.1.1150` or `8.5.1`.
    pub fn display(&self) -> &'static str {
        &self.entry.name
    }

    /// Whether the platform names one exact build rather than a release.
    pub fn is_exact_build(&self) -> bool {
        self.entry.build.is_some()
    }

    /// `[8, 5, 1]`: the release, also the compatibility mode the platform
    /// runs a configuration in that names none.
    pub fn release(&self) -> [u32; 3] {
        self.entry.release
    }

    /// The release as a platform (itself for a release).
    pub fn release_platform(&self) -> Result<Platform> {
        if self.entry.build.is_none() {
            return Ok(*self);
        }
        parse(&release_name(self.entry.release))
    }

    /// The platform's own compatibility level, packed as the rows store it:
    /// `80327`, `80501`.
    pub fn compatibility_packed(&self) -> u32 {
        let [major, minor, patch] = self.entry.release;
        major * 10000 + minor * 100 + patch
    }

    /// The platform's own compatibility mode as `Configuration.xml` spells
    /// it: `Version8_3_27`, `Version8_5_1`.
    pub fn compatibility_mode(&self) -> String {
        let [major, minor, patch] = self.entry.release;
        format!("Version{major}_{minor}_{patch}")
    }

    /// Whether the build writes `ButtonImportance` `Main` on a default button
    /// whose record holds the code of no importance of its own (see
    /// [`CONSTANT_DEFAULT_BUTTON_IMPORTANCE`]).
    pub fn writes_default_button_importance(&self) -> bool {
        self.entry.default_button_importance
    }

    /// The bundled profile of an exact build; `None` for a release.
    pub fn profile(&self) -> Option<&'static EffectiveProfile> {
        self.entry.profile.as_ref()
    }
}

/// Parses a platform version: an exact build (`8.3.27.2214`) or a release
/// (`8.3.27`) the registry knows.
pub fn parse(text: &str) -> Result<Platform> {
    let text = text.trim();
    let registry = registry()?;
    let components = text
        .split('.')
        .map(|part| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse::<u32>().ok())
                .flatten()
        })
        .collect::<Option<Vec<_>>>();
    let Some(components) = components.filter(|parts| matches!(parts.len(), 3 | 4)) else {
        bail!(
            "`{text}` is not a platform version: name a release such as 8.3.27 or an exact build such as 8.3.27.2214 ({})",
            known_list(registry)
        );
    };
    let canonical = components
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".");
    if let Some(entry) = registry
        .entries
        .iter()
        .find(|entry| entry.name == canonical)
    {
        return Ok(Platform { entry });
    }
    if registry.unsupported.contains(&canonical) {
        bail!(
            "platform {canonical} is not supported: ibcmd-rs reads and writes only the XML of the platforms it knows ({})",
            known_list(registry)
        );
    }
    if registry.ambiguous_releases.contains(&canonical) {
        bail!(
            "the known builds of {canonical} differ in their XML format, form layout or features: name the exact build ({})",
            known_list(registry)
        );
    }
    let what = if components.len() == 4 {
        "build"
    } else {
        "release"
    };
    bail!(
        "unknown platform {what} {canonical}: ibcmd-rs knows {}",
        known_list(registry)
    )
}

/// The release that reads and writes an XML format: 2.20 -> 8.3.27, 2.21 ->
/// 8.5.1. A format no known release writes, or one several releases write,
/// is refused: only a platform named outright tells them apart.
pub fn for_xml_format(xml: XmlFormat) -> Result<Platform> {
    let registry = registry()?;
    let mut releases = registry
        .entries
        .iter()
        .filter(|entry| entry.build.is_none() && entry.xml == xml);
    match (releases.next(), releases.next()) {
        (Some(entry), None) => Ok(Platform { entry }),
        (None, _) => bail!(
            "no known platform writes XML {}: ibcmd-rs knows {}",
            xml.as_str(),
            known_list(registry)
        ),
        (Some(first), Some(second)) => bail!(
            "XML {} is written by more than one known release ({}, {}, ...): name the platform",
            xml.as_str(),
            first.name,
            second.name
        ),
    }
}

/// The release of an XML dialect as the metadata contexts carry it: `2.20`
/// is 8.3.27's and any other dialect 8.5.1's, the way their
/// `is_xml_2_21()` reads it.
///
/// # Panics
///
/// Only when the bundled registry lacks the 8.3.27 or the 8.5.1 release,
/// which its tests rule out.
pub fn of_xml_dialect(dialect: &str) -> Platform {
    let xml = if dialect == XmlFormat::V2_20.as_str() {
        XmlFormat::V2_20
    } else {
        XmlFormat::V2_21
    };
    for_xml_format(xml).expect("the bundled platform registry maps XML 2.20 and 2.21")
}

/// Every known build and release, builds first, each in version order.
pub fn known() -> Result<Vec<Platform>> {
    Ok(registry()?
        .entries
        .iter()
        .map(|entry| Platform { entry })
        .collect())
}

/// The uuid of a named feature, as the builds that know it declare it.
pub fn feature_uuid(name: &str) -> Result<&'static str> {
    registry()?
        .entries
        .iter()
        .flat_map(|entry| &entry.features)
        .find(|feature| feature.name == name)
        .map(|feature| feature.uuid.as_str())
        .ok_or_else(|| anyhow!("no known platform declares the feature `{name}`"))
}

fn known_list(registry: &Registry) -> String {
    let builds = registry
        .entries
        .iter()
        .filter(|entry| entry.build.is_some())
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    let releases = registry
        .entries
        .iter()
        .filter(|entry| entry.build.is_none())
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    format!(
        "releases {}; builds {}",
        releases.join(", "),
        builds.join(", ")
    )
}

#[cfg(test)]
mod tests;
