//! The platform registry: every 1C:Enterprise build ibcmd-rs knows, and what
//! each one implies for the XML it reads and writes and for the rows it
//! stores.
//!
//! One place answers "what does platform X do":
//!
//! - its XML format: 8.3.x reads and writes 2.20, 8.5.x 2.21
//!   ([`PlatformSpec::xml_version`]);
//! - the stored layout of its managed forms and of the colours and fonts of
//!   other rows: the 8.3 one, or the one 8.5.1 introduced
//!   ([`PlatformSpec::form_layout`]);
//! - the features a configuration saved by it may list in its `version` row,
//!   such as the 8.5.1 palette colours ([`PlatformSpec::features`]);
//! - whether direct MSSQL activation is evidenced for it
//!   ([`PlatformSpec::live_activation`]).
//!
//! The builds and what they declare live in `profiles/platform/*.json`: the
//! bundled profiles `profile_registry` compiles in, whose MSSQL write
//! capabilities `MssqlNativePlatformProfile` checks. The `ibcmd-platform`
//! crate reads the same declarations -- the constants `platform.xml_format`,
//! `platform.form_layout` and `platform.feature.<name>` -- instead of
//! repeating them in code, and owns the registry of builds and releases; its
//! [`Platform`] is what code outside the root (the metadata model) reads.
//! This module wraps each one in a [`PlatformSpec`] with what only the root
//! knows: the command line's XML selector and the MSSQL capabilities. A build
//! whose profile declares no XML format (the 8.3.24.1819 seed) is known but
//! not supported.
//!
//! A platform is named by its full version: an exact build (`8.3.27.2214`)
//! or a release (`8.3.27`), which stands for every known build of it and
//! exists only while they agree on what they declare. A version the registry
//! does not know is refused with the list of known ones, never mapped to the
//! nearest one.
//!
//! # Naming, and what a new build needs
//!
//! Code that depends on the platform is named after the FULL version in which
//! its subject first appeared -- `layout_8_5_1.rs` (in `crates/ibcmd-form-body`),
//! `FormFactsV8_5_1`, [`FormLayout::V8_5_1`] -- and code of an XML dialect
//! after the XML format (`mssql_dump/form/xml_2_21_writer.rs`), never after a
//! nickname such as "85". When a new build arrives, say 8.5.4:
//!
//! - nothing changed for ibcmd-rs: one registry entry,
//!   `profiles/platform/8.5.4.<build>.json` listed in
//!   `profile_registry::BUNDLED_PROFILES`, declaring the XML format, form
//!   layout and features it shares with 8.5.1;
//! - its stored layout changed: in addition a delta module
//!   `layout_8_5_4.rs` beside `layout_8_5_1.rs` (and `load_8_5_4.rs`) converting
//!   between it and the 8.5.1 layout, a `FormLayout::V8_5_4`, and the entry
//!   declares `"platform.form_layout": "8.5.4"`;
//! - its XML format changed (say to 2.22): `xml_2_22_*` modules, an
//!   `InfobaseConfigSourceVersion::V2_22` and an `ibcmd_platform::XmlFormat::V2_22`,
//!   and the entry declares
//!   `"platform.xml_format": "2.22"`.

use std::fmt;
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use clap::ValueEnum;
use ibcmd_core::profile::{CapabilityId, CapabilityState};
use serde::{Serialize, Serializer};

use crate::legacy_version::InfobaseConfigSourceVersion;
use crate::mssql_platform_profile::{CAPABILITY_MAIN_WRITE, MssqlNativePlatformProfile};

// The portable registry -- the builds, the releases standing for them and
// what each declares -- is the `ibcmd-platform` crate's; its public paths are
// these.
pub use ibcmd_platform::{
    CONSTANT_DEFAULT_BUTTON_IMPORTANCE, CONSTANT_FEATURE_PREFIX, CONSTANT_FORM_LAYOUT,
    CONSTANT_XML_FORMAT, FEATURE_PALETTE_COLORS, FormLayout, Platform, PlatformFeature, XmlFormat,
    feature_uuid,
};

/// Help text of the `--platform` flag.
pub const PLATFORM_FLAG_HELP: &str = "Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21";

/// One portable platform with what the root adds to it.
struct Entry {
    platform: Platform,
    xml: InfobaseConfigSourceVersion,
    native_profile: Option<MssqlNativePlatformProfile>,
    live_activation: bool,
}

/// The portable registry's entries, in its order.
struct Registry {
    entries: Vec<Entry>,
}

static REGISTRY: OnceLock<std::result::Result<Registry, String>> = OnceLock::new();

fn registry() -> Result<&'static Registry> {
    REGISTRY
        .get_or_init(|| build_registry().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow!("{error}"))
}

/// The root entries. An error already says that the bundled platform
/// registry is invalid.
fn build_registry() -> Result<Registry> {
    let platforms = ibcmd_platform::known()?;
    let main_write = CapabilityId::parse(CAPABILITY_MAIN_WRITE).map_err(|error| {
        anyhow!(
            "the bundled platform registry is invalid: invalid capability id `{CAPABILITY_MAIN_WRITE}`: {error}"
        )
    })?;
    let entries = platforms
        .into_iter()
        .map(|platform| {
            let profile = platform.profile();
            let native_profile = profile.and_then(|profile| {
                MssqlNativePlatformProfile::value_variants()
                    .iter()
                    .copied()
                    .find(|native| native.id() == profile.id.as_str())
            });
            let live_activation = profile.is_some_and(|profile| {
                profile
                    .capabilities
                    .get(&main_write)
                    .is_some_and(|state| state.value == CapabilityState::Supported)
            });
            Entry {
                platform,
                xml: xml_version(platform.xml_format()),
                native_profile,
                live_activation,
            }
        })
        .collect();
    Ok(Registry { entries })
}

fn xml_version(xml: XmlFormat) -> InfobaseConfigSourceVersion {
    match xml {
        XmlFormat::V2_20 => InfobaseConfigSourceVersion::V2_20,
        XmlFormat::V2_21 => InfobaseConfigSourceVersion::V2_21,
    }
}

fn xml_format(xml: InfobaseConfigSourceVersion) -> XmlFormat {
    match xml {
        InfobaseConfigSourceVersion::V2_20 => XmlFormat::V2_20,
        InfobaseConfigSourceVersion::V2_21 => XmlFormat::V2_21,
    }
}

/// The spec of a portable platform.
fn spec(platform: Platform) -> Result<PlatformSpec> {
    registry()?
        .entries
        .iter()
        .find(|entry| entry.platform == platform)
        .map(|entry| PlatformSpec { entry })
        .ok_or_else(|| anyhow!("the root platform registry has no entry for {platform}"))
}

/// A known platform: an exact build or a release.
///
/// Cheap to copy; two specs are equal when they name the same entry.
#[derive(Clone, Copy)]
pub struct PlatformSpec {
    entry: &'static Entry,
}

impl PartialEq for PlatformSpec {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.entry, other.entry)
    }
}

impl Eq for PlatformSpec {}

impl fmt::Debug for PlatformSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PlatformSpec")
            .field(&self.display())
            .finish()
    }
}

impl fmt::Display for PlatformSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.display())
    }
}

impl Serialize for PlatformSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.display())
    }
}

/// The portable platform a spec wraps.
impl From<PlatformSpec> for Platform {
    fn from(spec: PlatformSpec) -> Self {
        spec.platform()
    }
}

/// The spec of a portable platform, such as the one a metadata context's
/// `platform()` returns.
///
/// # Panics
///
/// Never for a platform the portable registry returned: the root has an
/// entry for each, and its own additions cannot fail.
impl From<Platform> for PlatformSpec {
    fn from(platform: Platform) -> Self {
        spec(platform).expect("the root platform registry wraps every portable platform")
    }
}

impl PlatformSpec {
    /// The portable platform, without what only the root knows.
    pub fn platform(&self) -> Platform {
        self.entry.platform
    }

    /// The XML format the platform reads and writes: 2.20 for 8.3.x, 2.21 for
    /// 8.5.x.
    pub fn xml_version(&self) -> InfobaseConfigSourceVersion {
        self.entry.xml
    }

    /// The platform's own stored layout of forms, colours and fonts; a
    /// configuration kept in an older compatibility mode may store the
    /// previous one ([`FormLayout::stored`]).
    pub fn form_layout(&self) -> FormLayout {
        self.entry.platform.form_layout()
    }

    /// The features beyond older platforms' that the platform knows, by name.
    pub fn features(&self) -> &'static [PlatformFeature] {
        self.entry.platform.features()
    }

    /// One feature by name, when the platform knows it.
    pub fn feature(&self, name: &str) -> Option<&'static PlatformFeature> {
        self.entry.platform.feature(name)
    }

    /// The version as given: `8.5.1.1150` or `8.5.1`.
    pub fn display(&self) -> &'static str {
        self.entry.platform.display()
    }

    /// Whether the spec names one exact build rather than a release.
    pub fn is_exact_build(&self) -> bool {
        self.entry.platform.is_exact_build()
    }

    /// `[8, 5, 1]`: the release, also the compatibility mode the platform
    /// runs a configuration in that names none.
    pub fn release(&self) -> [u32; 3] {
        self.entry.platform.release()
    }

    /// The release as a spec (itself for a release).
    pub fn release_spec(&self) -> Result<PlatformSpec> {
        spec(self.entry.platform.release_platform()?)
    }

    /// The platform's own compatibility level, packed as the rows store it:
    /// `80327`, `80501`.
    pub fn compatibility_packed(&self) -> u32 {
        self.entry.platform.compatibility_packed()
    }

    /// The platform's own compatibility mode as `Configuration.xml` spells
    /// it: `Version8_3_27`, `Version8_5_1`.
    pub fn compatibility_mode(&self) -> String {
        self.entry.platform.compatibility_mode()
    }

    /// Whether the build writes `ButtonImportance` `Main` on a default button
    /// whose record holds the code of no importance of its own (see
    /// [`CONSTANT_DEFAULT_BUTTON_IMPORTANCE`]).
    pub fn writes_default_button_importance(&self) -> bool {
        self.entry.platform.writes_default_button_importance()
    }

    /// The native MSSQL profile of an exact build, when one is declared.
    pub fn native_profile(&self) -> Option<MssqlNativePlatformProfile> {
        self.entry.native_profile
    }

    /// Whether direct MSSQL activation of a staged configuration (without
    /// native ibcmd) is evidenced for this exact build: its profile declares
    /// `mssql.main.write` supported. A release is never enough, because the
    /// activation verifies the exact build it talks to.
    pub fn live_activation(&self) -> bool {
        self.entry.live_activation
    }
}

static EXPORT_DEFAULT_BUTTON_IMPORTANCE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Records the platform an export command runs for, once, at its start: the
/// writers of the XML spell a few things by the build (the XML format alone
/// does not tell 8.5.1.1150 from 8.5.1.1529). With no platform named, or a
/// release, the export keeps the spelling of the oldest build the registry
/// knows for the format.
pub fn note_export_platform(platform: PlatformSpec) {
    EXPORT_DEFAULT_BUTTON_IMPORTANCE.store(
        platform.writes_default_button_importance(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// Restore the oldest registered build's spelling when only a dialect is known.
#[doc(hidden)]
pub fn clear_export_platform() {
    EXPORT_DEFAULT_BUTTON_IMPORTANCE.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// See [`PlatformSpec::writes_default_button_importance`], for the platform
/// [`note_export_platform`] recorded.
pub fn export_writes_default_button_importance() -> bool {
    #[cfg(test)]
    if let Some(value) = TEST_DEFAULT_BUTTON_IMPORTANCE.with(std::cell::Cell::get) {
        return value;
    }
    EXPORT_DEFAULT_BUTTON_IMPORTANCE.load(std::sync::atomic::Ordering::Relaxed)
}

// Unit tests share the process: a test that depends on the export platform
// fixes it for its own thread instead of reading what a parallel test noted.
#[cfg(test)]
thread_local! {
    static TEST_DEFAULT_BUTTON_IMPORTANCE: std::cell::Cell<Option<bool>> =
        const { std::cell::Cell::new(None) };
}

/// Runs `body` with the export platform's default-button spelling fixed on
/// this thread (tests only).
#[cfg(test)]
pub(crate) fn with_export_default_button_importance<T>(value: bool, body: impl FnOnce() -> T) -> T {
    let previous = TEST_DEFAULT_BUTTON_IMPORTANCE.with(|cell| cell.replace(Some(value)));
    let result = body();
    TEST_DEFAULT_BUTTON_IMPORTANCE.with(|cell| cell.set(previous));
    result
}

/// Parses a platform version: an exact build (`8.3.27.2214`) or a release
/// (`8.3.27`) the registry knows.
pub fn parse(text: &str) -> Result<PlatformSpec> {
    spec(ibcmd_platform::parse(text)?)
}

/// The release that reads and writes an XML format: 2.20 -> 8.3.27, 2.21 ->
/// 8.5.1. A format no known release writes, or one several releases write,
/// is refused: only a platform named outright tells them apart.
pub fn for_xml_version(xml: InfobaseConfigSourceVersion) -> Result<PlatformSpec> {
    spec(ibcmd_platform::for_xml_format(xml_format(xml))?)
}

/// The release of an XML dialect as the metadata contexts carry it: `2.20`
/// is 8.3.27's and any other dialect 8.5.1's, the way their
/// `is_xml_2_21()` reads it ([`ibcmd_platform::of_xml_dialect`]).
///
/// # Panics
///
/// Only when the bundled registry lacks the 8.3.27 or the 8.5.1 release,
/// which its tests rule out.
#[doc(hidden)]
pub fn of_xml_dialect(dialect: &str) -> PlatformSpec {
    ibcmd_platform::of_xml_dialect(dialect).into()
}

/// Every known build and release, builds first, each in version order.
pub fn known() -> Result<Vec<PlatformSpec>> {
    Ok(registry()?
        .entries
        .iter()
        .map(|entry| PlatformSpec { entry })
        .collect())
}

/// For clap: parses `--platform` through the registry.
pub fn parse_flag(text: &str) -> std::result::Result<PlatformSpec, String> {
    parse(text).map_err(|error| format!("{error:#}"))
}

#[cfg(test)]
mod tests;
