//! EDT `Configuration.distr` (DistributionSupport model, version 6) and the
//! designer `ParentConfigurations.bin` it stands for.
//!
//! Witnessed with 1C:EDT 2026.1.3 (`1cedtcli import` of 8.3.27.2214 XML dumps,
//! then `export` back): EDT replaces the root `ParentConfigurations.bin` by
//! `Configuration.distr` and its export writes the designer file again with the
//! items in the `.distr` order. Six dumps fix the mapping: a configuration with
//! no supplier (`{6,0,0,0,1,0}`) and five with one supplier, covering every
//! item mode below. A file outside these shapes is refused, never guessed.
//!
//! Designer file: a BOM and one flat brace list
//! `{6,<distributive>,<suppliers>,` then per supplier
//! `<id>,<self store>,<version>,"<release>","<provider>","<name>",<count>,`
//! `<count>` × `<user mode>,<parent mode>,<user id>,<parent id>,` and a constant
//! trailer (`0,1,0` without a supplier, fifteen values with one).

use quick_xml::events::{BytesStart, Event};

/// The EDT file name beside `Configuration.mdo`.
pub(super) const EDT_FILE: &str = "Configuration.distr";

const NAMESPACE: &str = "http://g5.1c.ru/v8/dt/distribution/model";
const RULES: &str = "<rules existingIdenticalObjectsWithFreeModeKeepMode=\"true\" \
existingIdenticalObjectsWithWarningModeKeepMode=\"true\" \
existingDifferentObjectsWithFreeModeKeepMode=\"true\" \
existingDifferentObjectsWithFreeMode=\"ChangesAllowed\" \
existingDifferentObjectsWithWarningModeKeepMode=\"true\" \
existingDifferentObjectsWithWarningMode=\"ChangesAllowed\"/>";
const TRAILER_NONE: &[&str] = &["0", "1", "0"];
const TRAILER_ONE: &[&str] = &[
    "0", "0", "0", "1", "0", "0", "0", "1", "0", "1", "0", "1", "1", "1", "1",
];
// (designer value, EDT `userMode`); 0 is the omitted attribute.
const USER_MODES: &[(&str, Option<&str>)] = &[
    ("0", None),
    ("1", Some("ChangesAllowed")),
    ("2", Some("Cancelled")),
];
// (designer value, EDT `parentMode`); 0 is the omitted attribute.
const PARENT_MODES: &[(&str, Option<&str>)] = &[
    ("0", None),
    ("1", Some("WarningMode")),
    ("2", Some("ProtectMode")),
    ("-1", Some("NotIncludeInUserConfigByDefault")),
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    user: String,
    parent: String,
    user_mode: &'static str,
    parent_mode: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Supplier {
    id: String,
    self_store: bool,
    version: String,
    release: String,
    provider: String,
    name: String,
    items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Distribution {
    distributive: bool,
    supplier: Option<Supplier>,
}

/// The designer bytes for an EDT `.distr` file.
pub(super) fn bin_from_distr(source: &[u8]) -> Result<Vec<u8>, String> {
    let model = read_distr(source)?;
    // Every byte must be the spelling EDT writes for this model, so nothing in
    // the file is left unaccounted for.
    if write_distr(&model)? != source {
        return Err(format!(
            "{EDT_FILE} is not in the witnessed EDT spelling of its model"
        ));
    }
    Ok(write_bin(&model))
}

/// The EDT `.distr` bytes for a designer file, `None` when the file is outside
/// the witnessed shapes (it then stays a verbatim `ParentConfigurations.bin`).
pub(super) fn distr_from_bin(bin: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let Some(model) = read_bin(bin) else {
        return Ok(None);
    };
    if write_bin(&model) != bin {
        return Ok(None);
    }
    write_distr(&model).map(Some)
}

fn mode_value(table: &[(&'static str, Option<&str>)], edt: Option<&str>) -> Option<&'static str> {
    table.iter().find(|(_, name)| *name == edt).map(|(value, _)| *value)
}

fn mode_name(table: &[(&str, Option<&'static str>)], value: &str) -> Option<Option<&'static str>> {
    table.iter().find(|(v, _)| *v == value).map(|(_, name)| *name)
}

fn read_bin(bin: &[u8]) -> Option<Distribution> {
    let text = std::str::from_utf8(bin.strip_prefix(b"\xef\xbb\xbf")?).ok()?;
    let atoms = flat_atoms(text.strip_prefix('{')?.strip_suffix('}')?)?;
    let mut atoms = atoms.into_iter();
    let mut next = || atoms.next();
    if next()?.plain()? != "6" {
        return None;
    }
    let distributive = flag(next()?.plain()?)?;
    let supplier = match next()?.plain()? {
        "0" => None,
        "1" => {
            let id = next()?.plain()?.to_owned();
            if !identifier(&id) {
                return None;
            }
            let self_store = flag(next()?.plain()?)?;
            let version = next()?.plain()?.to_owned();
            if !identifier(&version) {
                return None;
            }
            let release = next()?.quoted()?;
            let provider = next()?.quoted()?;
            let name = next()?.quoted()?;
            let count: usize = next()?.plain()?.parse().ok()?;
            let mut items = Vec::with_capacity(count.min(1 << 20));
            for _ in 0..count {
                let user_mode = mode_value_of(USER_MODES, next()?.plain()?)?;
                let parent_mode = mode_value_of(PARENT_MODES, next()?.plain()?)?;
                let user = next()?.plain()?.to_owned();
                let parent = next()?.plain()?.to_owned();
                if !identifier(&user) || !identifier(&parent) {
                    return None;
                }
                items.push(Item {
                    user,
                    parent,
                    user_mode,
                    parent_mode,
                });
            }
            Some(Supplier {
                id,
                self_store,
                version,
                release,
                provider,
                name,
                items,
            })
        }
        _ => return None,
    };
    let trailer = if supplier.is_some() {
        TRAILER_ONE
    } else {
        TRAILER_NONE
    };
    for expected in trailer {
        if next()?.plain()? != *expected {
            return None;
        }
    }
    next().is_none().then_some(Distribution {
        distributive,
        supplier,
    })
}

fn identifier(value: &str) -> bool {
    formats_xml::children::parse_uuid(value)
        .is_ok_and(|uuid| formats_xml::children::format_uuid(&uuid) == value)
}

fn checked_identifier(value: String, field: &str) -> Result<String, String> {
    if !identifier(&value) {
        return Err(format!("{EDT_FILE}: malformed {field} UUID"));
    }
    Ok(value)
}

fn mode_value_of(table: &[(&'static str, Option<&str>)], value: &str) -> Option<&'static str> {
    table.iter().find(|(v, _)| *v == value).map(|(v, _)| *v)
}

fn flag(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

enum Atom<'a> {
    Plain(&'a str),
    Quoted(String),
}

impl Atom<'_> {
    fn plain(&self) -> Option<&str> {
        match self {
            Atom::Plain(value) => Some(value),
            Atom::Quoted(_) => None,
        }
    }

    fn quoted(&self) -> Option<String> {
        match self {
            Atom::Quoted(value) => Some(value.clone()),
            Atom::Plain(_) => None,
        }
    }
}

// One flat list: no nested braces, no white space, `""` inside quotes.
fn flat_atoms(text: &str) -> Option<Vec<Atom<'_>>> {
    let mut atoms = Vec::new();
    let mut rest = text;
    loop {
        if let Some(body) = rest.strip_prefix('"') {
            let mut value = String::new();
            let mut tail = body;
            loop {
                let end = tail.find('"')?;
                value.push_str(&tail[..end]);
                tail = &tail[end + 1..];
                match tail.strip_prefix('"') {
                    Some(more) => {
                        value.push('"');
                        tail = more;
                    }
                    None => break,
                }
            }
            atoms.push(Atom::Quoted(value));
            rest = tail;
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            let atom = &rest[..end];
            if atom.is_empty() || !atom.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                return None;
            }
            atoms.push(Atom::Plain(atom));
            rest = &rest[end..];
        }
        match rest.strip_prefix(',') {
            Some(more) => rest = more,
            None if rest.is_empty() => return Some(atoms),
            None => return None,
        }
    }
}

fn write_bin(model: &Distribution) -> Vec<u8> {
    let quote = |value: &str| format!("\"{}\"", value.replace('"', "\"\""));
    let flag = |value: bool| if value { "1" } else { "0" }.to_owned();
    let mut atoms = vec!["6".to_owned(), flag(model.distributive)];
    match &model.supplier {
        None => {
            atoms.push("0".to_owned());
            atoms.extend(TRAILER_NONE.iter().map(|a| (*a).to_owned()));
        }
        Some(supplier) => {
            atoms.push("1".to_owned());
            atoms.push(supplier.id.clone());
            atoms.push(flag(supplier.self_store));
            atoms.push(supplier.version.clone());
            atoms.push(quote(&supplier.release));
            atoms.push(quote(&supplier.provider));
            atoms.push(quote(&supplier.name));
            atoms.push(supplier.items.len().to_string());
            for item in &supplier.items {
                atoms.push(item.user_mode.to_owned());
                atoms.push(item.parent_mode.to_owned());
                atoms.push(item.user.clone());
                atoms.push(item.parent.clone());
            }
            atoms.extend(TRAILER_ONE.iter().map(|a| (*a).to_owned()));
        }
    }
    let mut out = b"\xef\xbb\xbf{".to_vec();
    out.extend_from_slice(atoms.join(",").as_bytes());
    out.push(b'}');
    out
}

fn escape(value: &str) -> Result<String, String> {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_control() => {
                return Err(format!(
                    "{EDT_FILE}: a control character in an attribute has no witnessed spelling"
                ));
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

fn write_distr(model: &Distribution) -> Result<Vec<u8>, String> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    out.push_str(&format!(
        "<distributionSupport:DistributionSupport xmlns:distributionSupport=\"{NAMESPACE}\" version=\"6\"{} updateAvailable=\"true\"",
        if model.distributive {
            " fileState=\"Distributive\""
        } else {
            ""
        }
    ));
    let Some(supplier) = &model.supplier else {
        out.push_str("/>\r\n");
        return Ok(out.into_bytes());
    };
    out.push_str(">\r\n");
    out.push_str(&format!(
        "  <parentConfigurationInfos id=\"{}\"{} configRelease=\"{}\" providerName=\"{}\" configName=\"{}\" configVersion=\"{}\">\r\n",
        escape(&supplier.id)?,
        if supplier.self_store {
            " storeMode=\"Self\""
        } else {
            ""
        },
        escape(&supplier.release)?,
        escape(&supplier.provider)?,
        escape(&supplier.name)?,
        escape(&supplier.version)?,
    ));
    out.push_str("    ");
    out.push_str(RULES);
    out.push_str("\r\n    <supportLanguages/>\r\n");
    for item in &supplier.items {
        out.push_str(&format!(
            "    <items userId=\"{}\" parentId=\"{}\"",
            escape(&item.user)?,
            escape(&item.parent)?
        ));
        if let Some(Some(mode)) = mode_name(PARENT_MODES, item.parent_mode) {
            out.push_str(&format!(" parentMode=\"{mode}\""));
        }
        if let Some(Some(mode)) = mode_name(USER_MODES, item.user_mode) {
            out.push_str(&format!(" userMode=\"{mode}\""));
        }
        out.push_str(" used=\"true\"/>\r\n");
    }
    out.push_str("  </parentConfigurationInfos>\r\n");
    out.push_str("</distributionSupport:DistributionSupport>\r\n");
    Ok(out.into_bytes())
}

fn attributes(element: &BytesStart<'_>) -> Result<Vec<(String, String)>, String> {
    element
        .attributes()
        .map(|attribute| {
            let attribute = attribute.map_err(|e| format!("{EDT_FILE}: {e}"))?;
            let key = std::str::from_utf8(attribute.key.as_ref())
                .map_err(|e| format!("{EDT_FILE}: {e}"))?
                .to_owned();
            let value = attribute
                .unescape_value()
                .map_err(|e| format!("{EDT_FILE}: {e}"))?
                .into_owned();
            Ok((key, value))
        })
        .collect()
}

fn take(attributes: &mut Vec<(String, String)>, key: &str) -> Option<String> {
    let index = attributes.iter().position(|(k, _)| k == key)?;
    Some(attributes.remove(index).1)
}

// Reads the model; `bin_from_distr` then requires the exact EDT spelling, so
// an attribute, element or value this reader does not take is refused there.
fn read_distr(source: &[u8]) -> Result<Distribution, String> {
    let fail = |what: &str| format!("{EDT_FILE}: {what}");
    let mut reader = quick_xml::Reader::from_reader(source);
    let mut distributive = false;
    let mut supplier: Option<Supplier> = None;
    loop {
        match reader.read_event().map_err(|e| fail(&e.to_string()))? {
            Event::Eof => break,
            Event::Start(element) | Event::Empty(element) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
                let mut attrs = attributes(&element)?;
                match name.as_str() {
                    "distributionSupport:DistributionSupport" => {
                        distributive = take(&mut attrs, "fileState").is_some();
                    }
                    "parentConfigurationInfos" => {
                        if supplier.is_some() {
                            return Err(fail("more than one supplier is not witnessed"));
                        }
                        let mut field = |key: &str| {
                            take(&mut attrs, key).ok_or_else(|| fail(&format!("no {key}")))
                        };
                        let id = checked_identifier(field("id")?, "id")?;
                        let release = field("configRelease")?;
                        let provider = field("providerName")?;
                        let name = field("configName")?;
                        let version = checked_identifier(field("configVersion")?, "configVersion")?;
                        let self_store = take(&mut attrs, "storeMode").is_some();
                        supplier = Some(Supplier {
                            id,
                            self_store,
                            version,
                            release,
                            provider,
                            name,
                            items: Vec::new(),
                        });
                    }
                    "items" => {
                        let supplier = supplier
                            .as_mut()
                            .ok_or_else(|| fail("an item outside its supplier"))?;
                        let user = checked_identifier(
                            take(&mut attrs, "userId").ok_or_else(|| fail("no userId"))?,
                            "userId",
                        )?;
                        let parent = checked_identifier(
                            take(&mut attrs, "parentId").ok_or_else(|| fail("no parentId"))?,
                            "parentId",
                        )?;
                        let user_mode =
                            mode_value(USER_MODES, take(&mut attrs, "userMode").as_deref())
                                .ok_or_else(|| fail("an unknown userMode"))?;
                        let parent_mode =
                            mode_value(PARENT_MODES, take(&mut attrs, "parentMode").as_deref())
                                .ok_or_else(|| fail("an unknown parentMode"))?;
                        supplier.items.push(Item {
                            user,
                            parent,
                            user_mode,
                            parent_mode,
                        });
                    }
                    "rules" | "supportLanguages" => {}
                    other => return Err(fail(&format!("an unknown element {other}"))),
                }
            }
            _ => {}
        }
    }
    Ok(Distribution {
        distributive,
        supplier,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE_BIN: &[u8] = b"\xef\xbb\xbf{6,0,0,0,1,0}";
    const NONE_DISTR: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<distributionSupport:DistributionSupport xmlns:distributionSupport=\"http://g5.1c.ru/v8/dt/distribution/model\" version=\"6\" updateAvailable=\"true\"/>\r\n";

    fn one_bin() -> Vec<u8> {
        let mut text = String::from(
            "\u{feff}{6,1,1,04bd6794-d318-44d6-93d5-2d85966a84a9,1,1b636e4c-2828-4ab7-936f-b3d9e3a69f50,\"3.1.11.309\",\"Фирма \"\"1С\"\"\",\"БиблиотекаСтандартныхПодсистем\",4,",
        );
        text.push_str("1,0,d95fcb37-fc65-466d-93e2-489f41683276,d95fcb37-fc65-466d-93e2-489f41683276,");
        text.push_str("0,2,0744e8be-747c-47bb-85f3-58b6f6f9b786,0744e8be-747c-47bb-85f3-58b6f6f9b786,");
        text.push_str("2,-1,b3aa97e1-baef-45f4-9179-052f35527a5e,b3aa97e1-baef-45f4-9179-052f35527a5e,");
        text.push_str("0,1,ee635706-6ee2-4956-8d58-a235f64687f6,b7ebf08a-d6be-42db-b28a-460df037d7ba,");
        text.push_str("0,0,0,1,0,0,0,1,0,1,0,1,1,1,1}");
        text.into_bytes()
    }

    #[test]
    fn a_configuration_without_a_supplier_has_the_short_file() {
        assert_eq!(
            distr_from_bin(NONE_BIN).unwrap().unwrap(),
            NONE_DISTR.as_bytes()
        );
        assert_eq!(bin_from_distr(NONE_DISTR.as_bytes()).unwrap(), NONE_BIN);
    }

    #[test]
    fn every_witnessed_item_mode_survives_both_directions() {
        let bin = one_bin();
        let distr = String::from_utf8(distr_from_bin(&bin).unwrap().unwrap()).unwrap();
        assert!(distr.contains(" storeMode=\"Self\" configRelease=\"3.1.11.309\" providerName=\"Фирма &quot;1С&quot;\""));
        assert!(distr.contains("d95fcb37-fc65-466d-93e2-489f41683276\" userMode=\"ChangesAllowed\" used=\"true\"/>"));
        assert!(distr.contains("0744e8be-747c-47bb-85f3-58b6f6f9b786\" parentMode=\"ProtectMode\" used=\"true\"/>"));
        assert!(distr.contains("parentMode=\"NotIncludeInUserConfigByDefault\" userMode=\"Cancelled\" used=\"true\"/>"));
        assert!(distr.contains("parentId=\"b7ebf08a-d6be-42db-b28a-460df037d7ba\" parentMode=\"WarningMode\" used=\"true\"/>"));
        assert_eq!(bin_from_distr(distr.as_bytes()).unwrap(), bin);
    }

    #[test]
    fn an_unwitnessed_shape_is_left_alone_or_refused() {
        // Another trailer: the designer file stays verbatim.
        let other = String::from_utf8(one_bin()).unwrap().replace(",1,1,1,1}", ",1,1,1,0}");
        assert_eq!(distr_from_bin(other.as_bytes()).unwrap(), None);
        // An unknown mode or an extra attribute in the EDT file is an error.
        let distr = String::from_utf8(distr_from_bin(&one_bin()).unwrap().unwrap()).unwrap();
        let mode = distr.replace("\"ProtectMode\"", "\"Locked\"");
        assert!(bin_from_distr(mode.as_bytes()).is_err());
        let extra = distr.replace(" used=\"true\"/>", " used=\"false\"/>");
        assert!(bin_from_distr(extra.as_bytes()).is_err());
        let rules = distr.replace("WithWarningMode=\"ChangesAllowed\"", "WithWarningMode=\"Locked\"");
        assert!(bin_from_distr(rules.as_bytes()).is_err());
    }
}
