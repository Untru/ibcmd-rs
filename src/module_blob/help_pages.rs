//! The HTML of a help page or an HTML template page as the platform stores
//! it, rebuilt from the page the export wrote.
//!
//! The exporter's `rewrite_help_links` turns the stored page into the file:
//! storage links become readable names, the attachment folder loses its
//! storage prefix and CRLF becomes LF. This is its inverse, and the page it
//! returns is checked by running that same reader over it: a page the reader
//! would not turn back into the very file it came from is refused rather than
//! stored.
//!
//! Names resolve against the source tree. A link to an object the tree does
//! not have is stored as the file spells it, which is what the platform
//! stores; a name the tree has under another spelling, a common picture it
//! lacks, a standard picture no table names, or any name at all without a
//! tree to look in, refuses the page.

use super::*;

/// The document an object's help is addressed by, and the folder prefix of a
/// help page's attachments: every stored help link of both corpora is
/// `../id<target>/<this>` (1 988 БСП, 10 876 ERP УХ, with or without an
/// anchor after it), and every attachment a help page names is
/// `<this>_files/<name>` (88 ERP УХ).
const HELP_DOCUMENT_ID: &str = "038b5c85-fb1c-4082-9c4c-e69f8928bf3a";
/// The folder prefix of an HTML template page's attachments (43 of 43 ERP УХ).
const HTML_TEMPLATE_DOCUMENT_ID: &str = "8eb4fad1-1fa6-403e-970f-2c12dbb43e23";
/// The second half of a stored picture reference,
/// `../../mdpicture/id<picture>/<this>` (354 БСП, 481 ERP УХ).
const PICTURE_VARIANT_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Which kind of HTML document a page belongs to: it decides the prefix the
/// platform stores on the page's attachment folder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HtmlPageOwner {
    /// An object's, a form's or the configuration's `Ext/Help`.
    Help,
    /// An `HTMLDocument` template's `Ext/Template`.
    Template,
}

impl HtmlPageOwner {
    const fn document_id(self) -> &'static str {
        match self {
            Self::Help => HELP_DOCUMENT_ID,
            Self::Template => HTML_TEMPLATE_DOCUMENT_ID,
        }
    }
}

/// The bytes the platform stores for a page the export wrote as `page`.
///
/// Only an attribute value is rewritten, and only one spelled exactly the way
/// the exporter spells it: `<Reference>/Help` or `<Reference>/Help#<anchor>` on
/// an `<a>` start tag, `CommonPicture.<Name>` or `StdPicture.<Name>` anywhere,
/// and `_files/<name>`. Everything else -- a raw `../id…` link the platform left
/// unresolved, a `_/Help` it stored literally -- is copied through. A page that
/// is not UTF-8 is stored as it is, which is also how the exporter writes it.
///
/// Measured with `audit-help-writer`: every help and `HTMLDocument` template
/// of БСП (642) and ERP УХ (6 446) is written without a refusal and reads back
/// into the exported files; 642 and 6 423 of the rows are the stored bytes.
pub fn html_page_storage_bytes(
    page: &[u8],
    owner: HtmlPageOwner,
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<u8>> {
    let Ok(text) = std::str::from_utf8(page) else {
        return Ok(page.to_vec());
    };
    // The names this page resolved, by uuid. The reader's own index is built
    // from the database; for the round-trip check these are the entries that
    // decide what it reads back.
    let mut names = BTreeMap::<String, String>::new();
    let text = text.replace('\n', "\r\n");
    let mut stored = String::with_capacity(text.len() + text.len() / 8);
    let mut offset = 0usize;
    while let Some(relative) = text[offset..].find("=\"") {
        let value_start = offset + relative + 2;
        let Some(value_length) = text[value_start..].find('"') else {
            break;
        };
        let value_end = value_start + value_length;
        stored.push_str(&text[offset..value_start]);
        let value = &text[value_start..value_end];
        match storage_attribute_value(&text, value_start, value, owner, source, &mut names)? {
            Some(rewritten) => stored.push_str(&rewritten),
            None => stored.push_str(value),
        }
        stored.push('"');
        offset = value_end + 1;
    }
    stored.push_str(&text[offset..]);

    let read_back = crate::mssql_dump::rewrite_help_links(stored.as_bytes(), &names);
    if read_back != page {
        let at = read_back
            .iter()
            .zip(page)
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| read_back.len().min(page.len()));
        let window = |bytes: &[u8]| {
            let start = at.saturating_sub(40).min(bytes.len());
            String::from_utf8_lossy(&bytes[start..(at + 60).min(bytes.len())]).to_string()
        };
        return Err(anyhow!(
            "the exporter would not read the stored page back into this file: at byte {at} it reads `{}` where the file has `{}`",
            window(&read_back),
            window(page)
        ));
    }
    Ok(stored.into_bytes())
}

/// What the platform stores for one attribute value, or `None` when it stores
/// the value as the file spells it.
fn storage_attribute_value(
    text: &str,
    value_start: usize,
    value: &str,
    owner: HtmlPageOwner,
    source: Option<&MetadataSourceContext>,
    names: &mut BTreeMap<String, String>,
) -> Result<Option<String>> {
    if let Some(rest) = value.strip_prefix("_files/") {
        return Ok(Some(format!("{}_files/{rest}", owner.document_id())));
    }
    // A standard picture is stored by the negative index or the uuid the
    // exporter names it from -- the same two tables, read the other way.
    if value.starts_with("StdPicture.") {
        if let Some(index) = crate::mssql_dump::help_standard_picture_negative_index(value) {
            return Ok(Some(format!("../../mdpicture/idn-{index}")));
        }
        let uuid = crate::mssql_dump::standard_picture_uuid(value)
            .ok_or_else(|| anyhow!("help picture `{value}` names no known standard picture"))?;
        return Ok(Some(format!(
            "../../mdpicture/id{uuid}/{PICTURE_VARIANT_ID}"
        )));
    }
    if value.starts_with("CommonPicture.") {
        // No page of either corpus names a common picture the tree lacks, so
        // nothing says what the platform stores for one: refused.
        let uuid = match help_reference(source, value, "help picture")? {
            HelpReference::Resolved(uuid) => uuid,
            HelpReference::Absent => {
                return Err(anyhow!("help picture `{value}` is not in the source tree"));
            }
        };
        names.insert(uuid.clone(), value.to_string());
        return Ok(Some(format!(
            "../../mdpicture/id{uuid}/{PICTURE_VARIANT_ID}"
        )));
    }
    let (reference, fragment) = match value.split_once("/Help") {
        Some((reference, "")) => (reference, ""),
        Some((reference, fragment)) if fragment.starts_with('#') => (reference, fragment),
        _ => return Ok(None),
    };
    if !is_stored_link_anchor(text, value_start) || !is_help_link_reference(reference) {
        return Ok(None);
    }
    // A link to an object the configuration does not have is stored as the
    // readable name itself: all 89 ERP УХ helps that link one (`Subsystem.
    // ЦеныИСкидки/Help`, a form of a business process the tree lacks) hold it
    // verbatim, exactly like the `_/Help` that is no reference at all.
    match help_reference(source, reference, "help link")? {
        HelpReference::Resolved(uuid) => {
            names.insert(uuid.clone(), reference.to_string());
            Ok(Some(format!("../id{uuid}/{HELP_DOCUMENT_ID}{fragment}")))
        }
        HelpReference::Absent => Ok(None),
    }
}

/// Whether the attribute value at `value_start` sits in a lowercase `<a …>`
/// start tag: the only place the platform stores a readable link as a link.
///
/// The exporter names a stored link on `<a>` and `<A>` alike and never on any
/// other tag (`help_link_marker_is_anchor`); anywhere else a readable spelling
/// is what the platform stored. The platform is stricter still about the case:
/// ERP УХ holds 10 565 links stored from `<a>` and not one from `<A>`, and the
/// four `<A href="Report.…/Help">` of three reports' help name reports the
/// tree has and are stored readable.
fn is_stored_link_anchor(text: &str, value_start: usize) -> bool {
    crate::mssql_dump::help_link_marker_is_anchor(text, value_start)
        && text[..value_start]
            .rfind('<')
            .is_some_and(|tag| text[tag + 1..].starts_with('a'))
}

/// What a readable reference names in the source tree.
#[derive(Clone, Debug)]
pub(super) enum HelpReference {
    Resolved(String),
    /// No file of the tree declares it: the configuration has no such object.
    Absent,
}

fn help_reference(
    source: Option<&MetadataSourceContext>,
    reference: &str,
    what: &str,
) -> Result<HelpReference> {
    source
        .ok_or_else(|| anyhow!("{what} `{reference}` needs the source tree to resolve its uuid"))?
        .help_reference(reference)
        .with_context(|| format!("{what} `{reference}` does not resolve"))
}

/// Whether `reference` is spelled like a metadata reference the exporter
/// writes for a link: `<Class>.<Name>` pairs whose first class is a
/// configuration class. A value that is not -- `_/Help` is stored literally by
/// one ERP УХ form -- is left as it is.
fn is_help_link_reference(reference: &str) -> bool {
    let parts = reference.split('.').collect::<Vec<_>>();
    parts.len() >= 2
        && parts.len() % 2 == 0
        && parts.iter().all(|part| {
            !part.is_empty()
                && !part
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '/' | '\\' | '"' | '#'))
        })
        && (parts[0] == "Configuration" || metadata_reference_source_folder(reference).is_some())
}

impl MetadataSourceContext {
    /// The uuid a readable help reference names, read from the source tree
    /// and memoised: an object, a nested subsystem, a form, a template, a
    /// recalculation, a command, a common picture or the configuration itself
    /// -- every name the exporter's help index holds.
    ///
    /// A reference whose file -- or whose owner's file -- the tree does not
    /// have is [`HelpReference::Absent`]; a file that is there and declares
    /// something else, or does not parse, is an error.
    fn help_reference(&self, reference: &str) -> Result<HelpReference> {
        if let Ok(cache) = self.help_references.lock()
            && let Some(found) = cache.get(reference)
        {
            return found.clone().map_err(|error| anyhow!(error));
        }
        let resolved = match self.read_help_reference_uuid(reference) {
            Ok(uuid) => Ok(HelpReference::Resolved(uuid)),
            Err(error) if is_not_found(&error) => Ok(HelpReference::Absent),
            Err(error) => Err(format!("{error:#}")),
        };
        if let Ok(mut cache) = self.help_references.lock() {
            cache.insert(reference.to_string(), resolved.clone());
        }
        resolved.map_err(|error| anyhow!(error))
    }

    fn read_help_reference_uuid(&self, reference: &str) -> Result<String> {
        let parts = reference.split('.').collect::<Vec<_>>();
        let (path, kind) = match parts.as_slice() {
            ["Configuration", _] => return self.resolve_configuration_uuid(reference),
            [_, _, "Command", _] => return self.resolve_command_reference_uuid(reference),
            [class, name] => {
                let (prefix, folder) = metadata_reference_source_folder(reference)
                    .ok_or_else(|| anyhow!("unsupported reference class `{class}`"))?;
                (
                    self.source_root.join(folder).join(format!("{name}.xml")),
                    prefix,
                )
            }
            ["Subsystem", ..] if parts.iter().step_by(2).all(|class| *class == "Subsystem") => {
                let mut path = self.source_root.join("Subsystems");
                let names = parts.iter().skip(1).step_by(2).collect::<Vec<_>>();
                for (index, name) in names.iter().enumerate() {
                    path = if index + 1 == names.len() {
                        path.join(format!("{name}.xml"))
                    } else {
                        path.join(name).join("Subsystems")
                    };
                }
                (path, "Subsystem")
            }
            [
                owner_class,
                owner,
                child_class @ ("Form" | "Template" | "Recalculation"),
                child,
            ] => {
                let owner_reference = format!("{owner_class}.{owner}");
                let (_, folder) = metadata_reference_source_folder(&owner_reference)
                    .ok_or_else(|| anyhow!("unsupported owner class `{owner_class}`"))?;
                let child_folder = match *child_class {
                    "Form" => "Forms",
                    "Template" => "Templates",
                    _ => "Recalculations",
                };
                (
                    self.source_root
                        .join(folder)
                        .join(owner)
                        .join(child_folder)
                        .join(format!("{child}.xml")),
                    *child_class,
                )
            }
            [owner_class, owner, child_class, child] => {
                return self.resolve_metadata_child_uuid(
                    &format!("{owner_class}.{owner}"),
                    child_class,
                    child,
                );
            }
            _ => return Err(anyhow!("unsupported reference shape")),
        };
        let xml = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let name = parts.last().copied().unwrap_or_default();
        // The tree is read on a case-insensitive file system, where
        // `Catalog.x` finds `Catalogs/X.xml`: the file must declare the very
        // class and name the reference spells.
        if properties.kind != kind || properties.name != name {
            return Err(anyhow!(
                "{} declares {}.{}",
                path.display(),
                properties.kind,
                properties.name
            ));
        }
        Ok(properties.uuid)
    }
}

/// Whether reading the tree failed because a file or a folder is not there.
fn is_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_an_exported_page_the_way_the_platform_does() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-help-pages-{}",
            Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(root.join("Catalogs")).unwrap();
        fs::create_dir_all(root.join("CommonPictures")).unwrap();
        fs::write(
            root.join("Catalogs").join("Товары.xml"),
            "<MetaDataObject><Catalog uuid=\"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\"><Properties><Name>Товары</Name></Properties></Catalog></MetaDataObject>",
        )
        .unwrap();
        fs::write(
            root.join("CommonPictures").join("Знак.xml"),
            "<MetaDataObject><CommonPicture uuid=\"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\"><Properties><Name>Знак</Name></Properties></CommonPicture></MetaDataObject>",
        )
        .unwrap();
        let source = MetadataSourceContext::new(root.clone());

        // A link on `<a>` keeps its anchor; one on `<area>` or `<A>` and a
        // value that is no reference are what the platform stored; pictures,
        // the attachment folder and the line ends take the stored spelling.
        let page = "<p><a href=\"Catalog.Товары/Help#Цены\">x</a><area href=\"Catalog.Товары/Help\"><a href=\"_/Help\">y</a><A href=\"Catalog.Товары/Help\">z</A></p>\n<img src=\"CommonPicture.Знак\"><img src=\"StdPicture.MoveUp\"><img src=\"StdPicture.Change\"><img src=\"_files/a b.png\">";
        let stored = html_page_storage_bytes(page.as_bytes(), HtmlPageOwner::Help, Some(&source));
        assert_eq!(
            String::from_utf8(stored.unwrap()).unwrap(),
            "<p><a href=\"../idaaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa/038b5c85-fb1c-4082-9c4c-e69f8928bf3a#Цены\">x</a><area href=\"Catalog.Товары/Help\"><a href=\"_/Help\">y</a><A href=\"Catalog.Товары/Help\">z</A></p>\r\n<img src=\"../../mdpicture/idbbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb/00000000-0000-0000-0000-000000000000\"><img src=\"../../mdpicture/idn-3\"><img src=\"../../mdpicture/id97b2cc97-d5c6-45fb-9824-9d6d73db21fe/00000000-0000-0000-0000-000000000000\"><img src=\"038b5c85-fb1c-4082-9c4c-e69f8928bf3a_files/a b.png\">"
        );
        let template = html_page_storage_bytes(
            b"<img src=\"_files/a.png\">",
            HtmlPageOwner::Template,
            Some(&source),
        );
        assert_eq!(
            template.unwrap(),
            b"<img src=\"8eb4fad1-1fa6-403e-970f-2c12dbb43e23_files/a.png\">"
        );

        // An object the tree does not have is stored by its readable name;
        // without a tree to look in, or -- where the file system finds it
        // anyway -- spelled in another case than the tree's, it is refused.
        let absent = "<a href=\"Catalog.Нет/Help\">";
        assert_eq!(
            html_page_storage_bytes(absent.as_bytes(), HtmlPageOwner::Help, Some(&source)).unwrap(),
            absent.as_bytes()
        );
        assert!(html_page_storage_bytes(absent.as_bytes(), HtmlPageOwner::Help, None).is_err());
        if cfg!(windows) {
            let other_case = "<a href=\"Catalog.товары/Help\">".as_bytes();
            assert!(
                html_page_storage_bytes(other_case, HtmlPageOwner::Help, Some(&source)).is_err()
            );
        }
        let _ = fs::remove_dir_all(root);
    }
}
