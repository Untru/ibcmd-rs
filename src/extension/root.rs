//! `Configuration.xml` of an extension: the root's `<Properties>` in the
//! order 8.3.27.2214 dumps them, and the root modules the extension extends.
//!
//! Positions in the root's `{68,…}` tuple, each set apart by an 8.3.27.2214
//! probe that changed one value (`_onecdec/make_extension_root_fixtures.py`):
//! 3 ScriptVariant, 4 DetailedInformation, 5 BriefInformation, 6 Copyright,
//! 7 VendorInformationAddress, 8 ConfigurationInformationAddress, 10 the
//! default language, 14 Vendor, 15 Version, 21 DefaultRunMode, 33 UsePurposes,
//! 38 InterfaceCompatibilityMode, 39 DefaultRoles, 42 NamePrefix,
//! 43 ConfigurationExtensionCompatibilityMode, 44 ConfigurationExtensionPurpose,
//! 49 KeepMappingToExtendedConfigurationObjectsByIDs.

use anyhow::{Context, Result, anyhow, bail};

use super::adoption::{self, Adoption};
use crate::{external::brace, legacy_version::InfobaseConfigSourceVersion};

/// The root's controllable properties by the uuid its header lists them
/// under (one probe each).
const DEFAULT_RUN_MODE: &str = "c6c6cdec-9de1-431f-b17a-e442eebf86c1";
const USE_PURPOSES: &str = "da648ef9-2f12-418f-8e2f-8956bc10a66f";
const DEFAULT_ROLES: &str = "6a447e3f-d9d7-4c97-a239-87e3fe6d8055";
const DEFAULT_LANGUAGE: &str = "15e3462b-bc9b-40cd-9d9f-dc0922f84560";
const INTERFACE_COMPATIBILITY_MODE: &str = "cbd1f1ed-72d3-4da3-bd02-c643d9595b7d";
/// Fixtures `adopted/props_*` (both, then each alone).
const MODALITY_USE_MODE: &str = "3a5cca5c-0675-43df-80ca-2cf5163335f2";
const COMPATIBILITY_MODE: &str = "161bc4c0-4cc0-4382-a045-f58f25e24783";

/// Root modules and command interfaces an extension extends, in the order
/// the platform writes their `<xr:PropertyState>`.
const EXTENDED: [(&str, &str); 6] = [
    (
        "d22e852a-cf8a-4f77-8ccb-3548e7792bea",
        "ManagedApplicationModule",
    ),
    ("9b7bbbae-9771-46f2-9e4d-2489e0ffc702", "SessionModule"),
    (
        "a4a9c1e2-1e54-4c7f-af06-4ca341198fac",
        "ExternalConnectionModule",
    ),
    (
        "a78d9ce3-4e0c-48d5-9863-ae7342eedf94",
        "OrdinaryApplicationModule",
    ),
    ("7f676314-716a-4d54-8335-a71a3857b21c", "CommandInterface"),
    (
        "9dfcabbf-6a7a-48aa-8721-df5e78b367c6",
        "MainSectionCommandInterface",
    ),
];

const STATE_EXTENDED: u8 = 3;

/// `ApplicationUsePurpose` values in a use-purpose list.
const USE_PURPOSE_CLASS: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";
/// The Role metadata class in a default-roles list.
const ROLE_CLASS: &str = "157fa490-4ce9-11d4-9415-008048da11f9";

const ROOT_FIELDS: usize = 61;

/// A 1C string at the start of `text`: its value and the bytes it spans.
pub fn quoted(text: &str) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut value = String::new();
    let mut from = 1;
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            value.push_str(&text[from..i]);
            if bytes.get(i + 1) == Some(&b'"') {
                value.push('"');
                i += 2;
                from = i;
                continue;
            }
            return Some((value, i + 1));
        }
        i += 1;
    }
    None
}

fn string(field: &str) -> Result<String> {
    quoted(field)
        .map(|(value, _)| value)
        .ok_or_else(|| anyhow!("expected a string, found `{field}`"))
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.replace("\r\n", "\n").chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn simple(xml: &mut String, name: &str, value: &str) {
    if value.is_empty() {
        xml.push_str(&format!("\t\t\t<{name}/>\r\n"));
    } else {
        xml.push_str(&format!("\t\t\t<{name}>{}</{name}>\r\n", escape(value)));
    }
}

/// `{N,"lang","text",…}`, `{0}` when empty.
fn localized(xml: &mut String, name: &str, field: &str) -> Result<()> {
    let (items, _) = brace::fields(field, 0).ok_or_else(|| anyhow!("{name}: `{field}`"))?;
    let count: usize = items
        .first()
        .and_then(|count| count.parse().ok())
        .ok_or_else(|| anyhow!("{name}: `{field}`"))?;
    if count == 0 {
        xml.push_str(&format!("\t\t\t<{name}/>\r\n"));
        return Ok(());
    }
    xml.push_str(&format!("\t\t\t<{name}>\r\n"));
    for index in 0..count {
        let lang = string(items.get(1 + 2 * index).copied().unwrap_or_default())?;
        let content = string(items.get(2 + 2 * index).copied().unwrap_or_default())?;
        xml.push_str("\t\t\t\t<v8:item>\r\n");
        if lang.is_empty() {
            // The platform never writes the start/end pair (ИТК: `<v8:lang/>`).
            xml.push_str("\t\t\t\t\t<v8:lang/>\r\n");
        } else {
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape(&lang)
            ));
        }
        xml.push_str(&format!(
            "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
            escape(&content)
        ));
        xml.push_str("\t\t\t\t</v8:item>\r\n");
    }
    xml.push_str(&format!("\t\t\t</{name}>\r\n"));
    Ok(())
}

fn code<'a>(name: &str, field: &str, spellings: &[&'a str]) -> Result<&'a str> {
    field
        .parse::<usize>()
        .ok()
        .and_then(|index| spellings.get(index).copied())
        .ok_or_else(|| anyhow!("{name}: unknown value `{field}`"))
}

/// `80327` → `Version8_3_27`.
fn compatibility_mode(field: &str) -> Result<String> {
    let version: u32 = field
        .parse()
        .map_err(|_| anyhow!("compatibility mode `{field}`"))?;
    if !(80000..90000).contains(&version) {
        bail!("compatibility mode `{field}`");
    }
    Ok(format!(
        "Version{}_{}_{}",
        version / 10000,
        (version / 100) % 100,
        version % 100
    ))
}

/// `{N,{"#",<class>,<value>},…}`.
fn use_purposes(xml: &mut String, field: &str) -> Result<()> {
    let (items, _) = brace::fields(field, 0).ok_or_else(|| anyhow!("UsePurposes `{field}`"))?;
    xml.push_str("\t\t\t<UsePurposes>\r\n");
    for item in items.iter().skip(1) {
        let (value, _) = brace::fields(item, 0).ok_or_else(|| anyhow!("UsePurposes `{item}`"))?;
        if value.get(1) != Some(&USE_PURPOSE_CLASS) {
            bail!("UsePurposes `{item}`");
        }
        let purpose = code(
            "UsePurposes",
            value.get(2).copied().unwrap_or_default(),
            &["", "PlatformApplication", "MobilePlatformApplication"],
        )?;
        if purpose.is_empty() {
            bail!("UsePurposes `{item}`");
        }
        xml.push_str(&format!(
            "\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">{purpose}</v8:Value>\r\n"
        ));
    }
    xml.push_str("\t\t\t</UsePurposes>\r\n");
    Ok(())
}

/// `{0,N,{"#",<Role class>,{1,<role uuid>}},…}`.
fn default_roles(
    xml: &mut String,
    field: &str,
    name_of: &dyn Fn(&str) -> Option<String>,
) -> Result<()> {
    let (items, _) = brace::fields(field, 0).ok_or_else(|| anyhow!("DefaultRoles `{field}`"))?;
    let roles = items.get(2..).unwrap_or_default();
    if roles.is_empty() {
        xml.push_str("\t\t\t<DefaultRoles/>\r\n");
        return Ok(());
    }
    xml.push_str("\t\t\t<DefaultRoles>\r\n");
    for role in roles {
        let (reference, _) =
            brace::fields(role, 0).ok_or_else(|| anyhow!("DefaultRoles `{role}`"))?;
        let (target, _) = reference
            .get(2)
            .and_then(|target| brace::fields(target, 0))
            .ok_or_else(|| anyhow!("DefaultRoles `{role}`"))?;
        if reference.get(1) != Some(&ROLE_CLASS) {
            bail!("DefaultRoles `{role}`");
        }
        let uuid = target.get(1).copied().unwrap_or_default();
        let name = name_of(uuid).ok_or_else(|| anyhow!("DefaultRoles: no role `{uuid}`"))?;
        xml.push_str(&format!(
            "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">Role.{}</xr:Item>\r\n",
            escape(&name)
        ));
    }
    xml.push_str("\t\t\t</DefaultRoles>\r\n");
    Ok(())
}

/// The `Name`/`Synonym`/`Comment` lines the pipeline wrote from the header.
fn identity(xml: &str) -> Result<&str> {
    let start = xml
        .find("\t\t\t<Name>")
        .context("Configuration.xml has no <Name>")?;
    let comment = xml[start..]
        .find("\t\t\t<Comment")
        .map(|at| start + at)
        .context("Configuration.xml has no <Comment>")?;
    let end = xml[comment..]
        .find("\r\n")
        .map(|at| comment + at + 2)
        .context("unterminated <Comment>")?;
    Ok(&xml[start..end])
}

fn properties(
    xml: &str,
    tuple: &[&str],
    adoption: &Adoption,
    name_of: &dyn Fn(&str) -> Option<String>,
    captions: Option<(&str, &str)>,
) -> Result<String> {
    // A listed property at state 0 is not controlled (see `adopted`).
    let controlled = |uuid: &str| adoption.state(uuid).is_some_and(|state| state != 0);
    let mut out = String::from("\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\r\n");
    out.push_str(identity(xml)?);
    simple(
        &mut out,
        "ConfigurationExtensionPurpose",
        code(
            "ConfigurationExtensionPurpose",
            tuple[44],
            &["Patch", "Customization", "AddOn"],
        )?,
    );
    simple(
        &mut out,
        "KeepMappingToExtendedConfigurationObjectsByIDs",
        code(
            "KeepMappingToExtendedConfigurationObjectsByIDs",
            tuple[49],
            &["false", "true"],
        )?,
    );
    simple(&mut out, "NamePrefix", &string(tuple[42])?);
    simple(
        &mut out,
        "ConfigurationExtensionCompatibilityMode",
        &compatibility_mode(tuple[43])?,
    );
    if controlled(DEFAULT_RUN_MODE) {
        simple(
            &mut out,
            "DefaultRunMode",
            code(
                "DefaultRunMode",
                tuple[21],
                &["OrdinaryApplication", "ManagedApplication", "Auto"],
            )?,
        );
    }
    if controlled(USE_PURPOSES) {
        use_purposes(&mut out, tuple[33])?;
    }
    simple(
        &mut out,
        "ScriptVariant",
        code("ScriptVariant", tuple[3], &["English", "Russian"])?,
    );
    if controlled(DEFAULT_ROLES) {
        default_roles(&mut out, tuple[39], name_of)?;
    }
    simple(&mut out, "Vendor", &string(tuple[14])?);
    simple(&mut out, "Version", &string(tuple[15])?);
    if let Some((caption, short_caption)) = captions {
        localized(&mut out, "Caption", caption)?;
        localized(&mut out, "ShortCaption", short_caption)?;
    }
    if controlled(DEFAULT_LANGUAGE) {
        let language = name_of(tuple[10])
            .ok_or_else(|| anyhow!("DefaultLanguage: no language `{}`", tuple[10]))?;
        simple(&mut out, "DefaultLanguage", &format!("Language.{language}"));
    }
    localized(&mut out, "BriefInformation", tuple[5])?;
    localized(&mut out, "DetailedInformation", tuple[4])?;
    localized(&mut out, "Copyright", tuple[6])?;
    localized(&mut out, "VendorInformationAddress", tuple[7])?;
    localized(&mut out, "ConfigurationInformationAddress", tuple[8])?;
    if controlled(MODALITY_USE_MODE) {
        // Tuple member 36: 0 Use, 1 UseWithWarnings, 2 DontUse (the
        // configuration's own reading, `configuration_properties_evidence`).
        simple(
            &mut out,
            "ModalityUseMode",
            code(
                "ModalityUseMode",
                tuple[36],
                &["Use", "UseWithWarnings", "DontUse"],
            )?,
        );
    }
    if controlled(INTERFACE_COMPATIBILITY_MODE) {
        simple(
            &mut out,
            "InterfaceCompatibilityMode",
            code(
                "InterfaceCompatibilityMode",
                tuple[38],
                &[
                    "Version8_2",
                    "Version8_2EnableTaxi",
                    "TaxiEnableVersion8_2",
                    "Taxi",
                ],
            )?,
        );
    }
    if controlled(COMPATIBILITY_MODE) {
        // Tuple member 26, the configuration's own compatibility mode.
        simple(
            &mut out,
            "CompatibilityMode",
            &compatibility_mode(tuple[26])?,
        );
    }
    Ok(out)
}

fn property_states(adoption: &Adoption) -> String {
    let mut out = String::new();
    for (uuid, name) in EXTENDED {
        if adoption.state(uuid) == Some(STATE_EXTENDED) {
            out.push_str(&format!(
                "\t\t\t<xr:PropertyState>\r\n\t\t\t\t<xr:Property>{name}</xr:Property>\r\n\
                 \t\t\t\t<xr:State>Extended</xr:State>\r\n\t\t\t</xr:PropertyState>\r\n"
            ));
        }
    }
    out
}

/// Every property the root header lists is one this writer knows; anything
/// else is refused rather than dropped.
fn check_known(adoption: &Adoption) -> Result<()> {
    let known = [
        DEFAULT_RUN_MODE,
        USE_PURPOSES,
        DEFAULT_ROLES,
        DEFAULT_LANGUAGE,
        INTERFACE_COMPATIBILITY_MODE,
        MODALITY_USE_MODE,
        COMPATIBILITY_MODE,
    ];
    for (uuid, _) in &adoption.properties {
        if !known.contains(&uuid.as_str()) && !EXTENDED.iter().any(|(id, _)| id == uuid) {
            bail!("the root controls property `{uuid}`, which is not known");
        }
    }
    Ok(())
}

/// 8.5 writes the root tuple as `{76,…}` of 77 members, the first 61 of them
/// the `{68,…}` members; 8.3.27.2214 reads an 8.5-saved extension from those
/// (fixture `values_v85`).
const ROOT_FIELDS_V85: usize = 77;

/// Members 64 and 65 of the 8.5 `{76,…}` tuple: the root's `Caption` and
/// `ShortCaption`, which 8.5 writes after `Version` (fixtures
/// `v85_extension/*`; an older tuple has neither and 8.5 writes both empty).
const CAPTION: usize = 64;
const SHORT_CAPTION: usize = 65;
const NO_CAPTION: &str = "{0}";

/// The root's `Caption` and `ShortCaption` fields in `row` (`{0}` each for a
/// tuple older than 8.5's).
fn root_captions(row: &str) -> Result<(&str, &str)> {
    let tuple = full_root_tuple(row)?;
    if tuple.len() == ROOT_FIELDS_V85 {
        Ok((tuple[CAPTION], tuple[SHORT_CAPTION]))
    } else {
        Ok((NO_CAPTION, NO_CAPTION))
    }
}

fn full_root_tuple(row: &str) -> Result<Vec<&str>> {
    let start = row
        .find(ROOT_CLASS)
        .and_then(|at| row[at..].find("{1,").map(|open| at + open))
        .context("no root properties section")?;
    let (section, _) = brace::fields(row, start).context("unbalanced root section")?;
    let tuple_text = section.get(1).context("empty root properties section")?;
    let (tuple, _) = brace::fields(tuple_text, 0).context("unbalanced root tuple")?;
    Ok(tuple)
}

/// The members of the root's `{68,…}` tuple (or of the 8.5 `{76,…}` one).
fn root_tuple(row: &str) -> Result<Vec<&str>> {
    let mut tuple = full_root_tuple(row)?;
    let version = tuple
        .first()
        .and_then(|member| member.parse::<u32>().ok())
        .unwrap_or(0);
    // 8.3.27 writes 61 members, 8.5 77 (the 61 plus 16 appended); an older
    // tuple is a prefix of the 61 (ИР 7.77: `{66,…}` of 59). The writer reads
    // members up to OLDEST_MEMBERS - 1.
    match (version, tuple.len()) {
        (68, ROOT_FIELDS) | (76, ROOT_FIELDS_V85) => tuple.truncate(ROOT_FIELDS),
        (59..=67, len) if (OLDEST_MEMBERS..ROOT_FIELDS).contains(&len) => {}
        (_, len) => bail!("{{{version},…}} root tuple of {len} members"),
    }
    Ok(tuple)
}

/// The class of the root's properties section.
const ROOT_CLASS: &str = "{9cd510cd-abfc-11d4-9434-004095e12fc7,";

/// Members the writer reads: up to ExtensionCompatibility (49).
const OLDEST_MEMBERS: usize = 50;

/// `xml` (the pipeline's Configuration.xml of the extension) rewritten from
/// the root row `row`; `name_of` names an object of the extension by uuid.
/// Dialect 2.21 adds the root's captions (8.5.1.1529).
pub fn rewrite(
    xml: &str,
    row: &str,
    name_of: &dyn Fn(&str) -> Option<String>,
    source_version: InfobaseConfigSourceVersion,
) -> Result<String> {
    let tuple = root_tuple(row)?;
    let captions = match source_version {
        InfobaseConfigSourceVersion::V2_20 => None,
        _ => Some(root_captions(row)?),
    };
    let header = tuple[1]
        .find("{3,")
        .and_then(|at| adoption::parse(&tuple[1][at..]))
        .context("root header")?;
    if !header.adopted {
        bail!("the extension root is not adopted");
    }
    check_known(&header)?;

    let open = xml
        .find("\t\t<Properties>\r\n")
        .context("no <Properties>")?
        + "\t\t<Properties>\r\n".len();
    let close = xml.find("\t\t</Properties>").context("no </Properties>")?;
    let mut out = String::with_capacity(xml.len() + 1024);
    out.push_str(&xml[..open]);
    out.push_str(&properties(xml, &tuple, &header, name_of, captions)?);
    out.push_str(&xml[close..]);

    let states = property_states(&header);
    if !states.is_empty() {
        let at = out
            .find("\t\t</InternalInfo>")
            .context("no </InternalInfo> for the extended root modules")?;
        out.insert_str(at, &states);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_1c_strings_with_doubled_quotes() {
        assert_eq!(quoted("\"a\"\"b\",x"), Some(("a\"b".to_owned(), 6)));
        assert_eq!(quoted("\"\""), Some((String::new(), 2)));
        assert_eq!(quoted("x"), None);
    }

    #[test]
    fn an_empty_language_is_self_closed() {
        // 8.3.27.2214 never writes `<v8:lang></v8:lang>` (ИТК: five
        // `<v8:lang/>`, no pair); the root writer saves its XML directly.
        let mut xml = String::new();
        localized(&mut xml, "BriefInformation", "{1,\"\",\"Кратко\"}").unwrap();
        assert!(xml.contains("\t\t\t\t\t<v8:lang/>\r\n"), "{xml}");
        assert!(!xml.contains("<v8:lang></v8:lang>"), "{xml}");
    }

    #[test]
    fn spells_compatibility_modes() {
        assert_eq!(compatibility_mode("80310").unwrap(), "Version8_3_10");
        assert_eq!(compatibility_mode("80327").unwrap(), "Version8_3_27");
        assert!(compatibility_mode("0").is_err());
    }
}
