//! Attach the Subsystem COMMAND-INTERFACE SIDECAR (`CommandInterface.cmi` EDT /
//! `Ext/CommandInterface.xml` Designer) into the object's IR `command_interface` during
//! whole-config read (§1.0/§1.6). Mirror of [`crate::rights_read`]/[`crate::template_read`]
//! — "the descriptor read is metadata-only, the command-interface is a sibling file the
//! pipeline attaches" — with the §1.0 OPTIONAL-body shape of [`crate::template_read`]
//! (a Subsystem MAY carry a CommandInterface; absence is honest, never a hard error).
//!
//! # Why: edt/designer→cf needs the command-interface the way it needs the rights table
//! The cf Subsystem writer emits the descriptor `<uuid>` PLUS, when the subsystem carries a
//! command-interface, a separate BODY element `<uuid>.1` (the CommandInterface visibility
//! record — `formats_cf::assemble_cf` → `formats_cf::cmi_body`). That body is sourced from
//! `MetadataObject.command_interface`. The per-kind Subsystem descriptor connector reads only
//! the thin `.mdo`/`.xml` (name/synonym/content); the command-interface lives in a sidecar.
//! This pass reads it into `obj.command_interface` so `--to cf` can emit the `<uuid>.1` body.
//!
//! # Sidecar layout beside the descriptor (RE: s14_cmi corpus)
//! * **EDT** (`Subsystems/<Name>/<Name>.mdo`, dir-per-object; NESTED subsystems live under
//!   `Subsystems/<Parent>/Subsystems/<Name>/…`): sidecar SIBLING
//!   `<obj-dir>/CommandInterface.cmi` (ns `http://g5.1c.ru/v8/dt/cmi`).
//! * **Designer** (`Subsystems/<Name>.xml`, file-per-object): sidecar
//!   `Subsystems/<Name>/Ext/CommandInterface.xml` (ns `http://v8.1c.ru/8.3/xcf/extrnprops`).
//!
//! The path is DERIVED from the descriptor path exactly as the enumerator passed it, so a
//! nested subsystem resolves its sidecar beside its own `.mdo`/`.xml` (no flat-name collision).
//!
//! # §1.0 — OPTIONAL, and STRICT on shape
//! CommandInterface is OPTIONAL (only some subsystems carry it — s14's `ПодсистемаКИ` does;
//! most SSL subsystems do not) → a missing sidecar is a NO-OP (mirror `template_read`), never a
//! hard error. But when present it is parsed STRICTLY: only the four witnessed regions
//! (`commandsVisibility`/`commandsPlacement`/`commandsOrder`/`subsystemsOrder` + the Designer
//! derived `GroupsOrder`) are modelled; any OTHER top-level region or an unrecognized
//! visibility shape is a typed [`ConvertError::Read`] (never a silent skip that would let
//! `--to cf` fabricate a wrong `<uuid>.1` body).
//!
//! # Witnessed shape census (ERP designer_8.3.27, 245 sidecars ↔ 245 EDT `.cmi`)
//! * Region ORDER (both dialects, EDT names lowercased): `commandsVisibility?`,
//!   `commandsPlacement?`, `commandsOrder?`, `subsystemsOrder?`; Designer additionally emits
//!   `GroupsOrder` LAST — and only alongside `CommandsOrder` (164/164). Every present region is
//!   non-empty; 70 sidecars carry visibility ONLY, 16 order ONLY, 4 subsystemsOrder ONLY.
//! * Visibility: Designer `<Visibility>` = `<xr:Common>BOOL</xr:Common>` FIRST (1965/1965),
//!   then per-role `<xr:Value name="Role.X">BOOL</xr:Value>`× (102: 97 true / 5 false). EDT
//!   `<visible>` = empty (Common false, no roles) | `<common>true</common>` (never `false` —
//!   omission IS false) followed by `<for><value>true</value><role>Role.X</role></for>`×
//!   (`<value>` omitted == `false` — witnessed 5/5 against Designer).
//! * `subsystemsOrder`: EDT `<subsystems>PATH</subsystems>`× / Designer
//!   `<Subsystem>PATH</Subsystem>`× with PATH = `Subsystem.<Родитель>.Subsystem.<Дочерняя>`
//!   (27 carriers; the lists are byte-equal between dialects — §1.6 X by construction).
//! * Designer root carries `version="2.20|2.21"` (format version is a READ input, see
//!   [`crate::sidecar_version`]; the body shape is version-invariant across both corpora).

use std::path::{Path, PathBuf};

use formats_xml::Element;
use formats_xml::registry::Format;
use morph1c_core::ir::{
    CommandGroupFragment, CommandInterface, CommandVisibility, MetadataObject, RoleVisibility,
    SubsystemVisibility,
};
use morph1c_core::version::FormatVersion;

use crate::ConvertError;

/// Kinds whose object carries a command-interface sidecar (currently only `Subsystem`).
const CMI_SIDECAR_KINDS: &[&str] = &["Subsystem"];

/// EDT sidecar file name (dir-per-object sibling).
const EDT_CMI_FILE: &str = "CommandInterface.cmi";
/// Designer sidecar file name (under `<Name>/Ext/`).
const DESIGNER_CMI_FILE: &str = "CommandInterface.xml";

/// Designer `<Placement>`-текст ПОДСИСТЕМНЫХ сайдкаров — единственный witnessed (SSL 22/22 +
/// ERP 401/401). КОНФИГ-УРОВНЕВЫЕ cmi-сайдкары несут [`PLACEMENT_MANUAL`]
/// (`crate::config_interface_read`).
pub(crate) const PLACEMENT_AUTO: &str = "Auto";
/// Designer `<Placement>`-текст КОНФИГ-УРОВНЕВЫХ cmi-сайдкаров (witnessed ERP
/// `Ext/MainSectionCommandInterface.xml` 148/148; cf-код 1 — `formats_cf::cmi_body`).
pub(crate) const PLACEMENT_MANUAL: &str = "Manual";

/// Load the Subsystem command-interface (from its sidecar) into `obj.command_interface`.
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). The sidecar is resolved
/// beside it per the format layout. §1.0: the sidecar is OPTIONAL (absent ⇒ no-op — a Subsystem
/// need not carry a command-interface). When present it is parsed strictly (see module docs).
/// Non-Subsystem kinds and cf (container, no sidecar) are no-ops.
pub fn attach_command_interface(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if !CMI_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let path = match cmi_sidecar(format, descriptor_path) {
        Some(p) => p,
        None => return Ok(()), // cf: container — no file-per-object sidecar to attach.
    };
    // §1.0 OPTIONAL: a Subsystem MAY carry a command-interface; absence is honest (no-op),
    // never a hard error (unlike Role rights). Only some subsystems carry it.
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let ci = parse_command_interface(format, &bytes).map_err(|reason| ConvertError::Read {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason,
    })?;
    // §1.0: never silently overwrite an already-attached command-interface (the descriptor
    // read must not populate it — only this pass does).
    if obj.command_interface.is_some() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries a command-interface before the sidecar attach \
                     (unexpected — the descriptor projection must not populate it)"
                .into(),
        });
    }
    obj.command_interface = Some(ci);
    Ok(())
}

/// Write-side mirror of [`attach_command_interface`]: emit the Subsystem command-interface `obj`
/// carries beside its just-written descriptor `descriptor_out`, serialised BYTE-EXACT in the
/// target dialect (EDT `CommandInterface.cmi` / Designer `Ext/CommandInterface.xml`). No-op for
/// non-Subsystem kinds, cf, and subsystems with no command-interface. §1.0: typed I/O error only.
pub fn write_command_interface(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if !CMI_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let ci = match &obj.command_interface {
        Some(c) => c,
        None => return Ok(()), // OPTIONAL body — nothing attached, nothing to emit.
    };
    let path = match cmi_sidecar(format, descriptor_out) {
        Some(p) => p,
        None => return Ok(()), // cf: container.
    };
    let bytes = match format {
        Format::Edt => serialize_edt(ci),
        // Версия ТАРГЕТА — амбьентный round-trip-таргет (формат выхода — параметр).
        Format::Designer => serialize_designer(ci, crate::sidecar_version::write_target()),
        Format::Cf => return Ok(()),
    };
    crate::form_write::write_file(&path, &bytes)
}

/// EDT `.cmi` serializer — byte-exact inverse of [`parse_edt`] (RE: s14 `CommandInterface.cmi`
/// + ERP 245/245 + конфиг-уровневые `CommandInterface.cmi`/`MainSectionCommandInterface.cmi`):
/// NO BOM, 2-space indent, CRLF (incl. trailing), one `cmi:` namespace.
/// `<visible>` shapes: empty `<visible/>` (Common false, no roles); `<common>true</common>`
/// emitted only when TRUE (omission == false — witnessed, never `<common>false</common>`);
/// then `<for>` per role value (`<value>true</value>` only when TRUE, then `<role>`). Every
/// region is emitted ONLY when non-empty (witnessed ERP: 21 sidecars carry NO visibility region).
/// `pub(crate)`: конфиг-уровневые cmi-сайдкары (`crate::config_interface_read`) — ТОТ ЖЕ
/// EDT-диалект (у EDT placement-текста нет вовсе — Auto/Manual контекстно-выводим).
pub(crate) fn serialize_edt(ci: &CommandInterface) -> Vec<u8> {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    s.push_str("<cmi:CommandInterface xmlns:cmi=\"http://g5.1c.ru/v8/dt/cmi\">\r\n");
    if !ci.commands.is_empty() {
        s.push_str("  <commandsVisibility>\r\n");
        for c in &ci.commands {
            s.push_str("    <visibilityFragments>\r\n");
            s.push_str("      <command>");
            s.push_str(&c.command);
            s.push_str("</command>\r\n");
            if !c.common_visible && c.role_values.is_empty() {
                s.push_str("      <visible/>\r\n");
            } else {
                s.push_str("      <visible>\r\n");
                if c.common_visible {
                    s.push_str("        <common>true</common>\r\n");
                }
                for rv in &c.role_values {
                    s.push_str("        <for>\r\n");
                    if rv.visible {
                        s.push_str("          <value>true</value>\r\n");
                    }
                    s.push_str("          <role>");
                    s.push_str(&rv.role);
                    s.push_str("</role>\r\n        </for>\r\n");
                }
                s.push_str("      </visible>\r\n");
            }
            s.push_str("    </visibilityFragments>\r\n");
        }
        s.push_str("  </commandsVisibility>\r\n");
    }
    emit_subsystem_visibility_edt(&mut s, &ci.subsystems_visibility);
    // Regions in fixed emission order after visibility (RE: SSL Администрирование /
    // ОценкаПроизводительности + ERP census): commandsPlacement, commandsOrder,
    // subsystemsOrder — each present only when non-empty. `placementFragments`/
    // `orderFragments` share the `<group>` + `<commands>*` shape.
    emit_edt_region(
        &mut s,
        "commandsPlacement",
        "placementFragments",
        &ci.placement,
    );
    emit_edt_region(&mut s, "commandsOrder", "orderFragments", &ci.order);
    if !ci.subsystems_order.is_empty() {
        s.push_str("  <subsystemsOrder>\r\n");
        for sub in &ci.subsystems_order {
            s.push_str("    <subsystems>");
            s.push_str(sub);
            s.push_str("</subsystems>\r\n");
        }
        s.push_str("  </subsystemsOrder>\r\n");
    }
    s.push_str("</cmi:CommandInterface>\r\n");
    s.into_bytes()
}

/// Emit one EDT grouped region (`commandsPlacement`/`commandsOrder`) if it carries fragments.
/// 2-space indent, CRLF — matching the visibility region above.
fn emit_edt_region(s: &mut String, region: &str, frag: &str, frags: &[CommandGroupFragment]) {
    if frags.is_empty() {
        return;
    }
    s.push_str("  <");
    s.push_str(region);
    s.push_str(">\r\n");
    for f in frags {
        s.push_str("    <");
        s.push_str(frag);
        s.push_str(">\r\n      <group>");
        s.push_str(&f.group);
        s.push_str("</group>\r\n");
        for c in &f.commands {
            s.push_str("      <commands>");
            s.push_str(c);
            s.push_str("</commands>\r\n");
        }
        s.push_str("    </");
        s.push_str(frag);
        s.push_str(">\r\n");
    }
    s.push_str("  </");
    s.push_str(region);
    s.push_str(">\r\n");
}

/// Designer `CommandInterface.xml` serializer — byte-exact inverse of [`parse_designer`] (RE: s14
/// `Ext/CommandInterface.xml` + ERP 245/245): leading UTF-8 BOM, tab indent, CRLF, NO trailing
/// newline, four namespaces + `version=` ЗАДАННОЙ версии формата (2.20 ERP / 2.21 SSL — ns-блок
/// побайтно ОДИН И ТОТ ЖЕ, различается ТОЛЬКО `version=`). Both truth values emit an explicit
/// `<xr:Common>BOOL</xr:Common>`; role values follow as `<xr:Value name="Role.X">BOOL</xr:Value>`.
/// Every region is emitted ONLY when non-empty (witnessed ERP: 21 sidecars carry NO
/// CommandsVisibility); `SubsystemsOrder` sits between `CommandsOrder` and the derived
/// `GroupsOrder` (witnessed 18/18 co-carriers). Подсистемный сайдкар — `<Placement>Auto</…>`.
fn serialize_designer(ci: &CommandInterface, version: FormatVersion) -> Vec<u8> {
    serialize_designer_with_placement(ci, version, PLACEMENT_AUTO)
}

/// [`serialize_designer`] с ЯВНЫМ `<Placement>`-текстом — конфиг-уровневые cmi-сайдкары
/// (`crate::config_interface_read`) несут `Manual` (witnessed ERP 148/148).
pub(crate) fn serialize_designer_with_placement(
    ci: &CommandInterface,
    version: FormatVersion,
    placement_text: &str,
) -> Vec<u8> {
    let mut s = String::new();
    s.push('\u{FEFF}'); // UTF-8 BOM (Designer text sidecars carry it).
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    s.push_str(&format!(
        "<CommandInterface xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
         xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n",
    ));
    if !ci.commands.is_empty() {
        s.push_str("\t<CommandsVisibility>\r\n");
        for c in &ci.commands {
            s.push_str("\t\t<Command name=\"");
            s.push_str(&c.command);
            s.push_str("\">\r\n\t\t\t<Visibility>\r\n\t\t\t\t<xr:Common>");
            s.push_str(if c.common_visible { "true" } else { "false" });
            s.push_str("</xr:Common>\r\n");
            for rv in &c.role_values {
                s.push_str("\t\t\t\t<xr:Value name=\"");
                s.push_str(&rv.role);
                s.push_str("\">");
                s.push_str(if rv.visible { "true" } else { "false" });
                s.push_str("</xr:Value>\r\n");
            }
            s.push_str("\t\t\t</Visibility>\r\n\t\t</Command>\r\n");
        }
        s.push_str("\t</CommandsVisibility>\r\n");
    }
    emit_subsystem_visibility_designer(&mut s, &ci.subsystems_visibility);
    // Regions after visibility (Designer per-command TRANSPOSED shape). CommandsPlacement carries
    // `<Placement>TEXT</Placement>` per command (Auto — подсистемы, Manual — конфиг-уровень);
    // CommandsOrder does not.
    emit_designer_region(
        &mut s,
        "CommandsPlacement",
        Some(placement_text),
        &ci.placement,
    );
    emit_designer_region(&mut s, "CommandsOrder", None, &ci.order);
    // `SubsystemsOrder` — ПЕРЕД производным GroupsOrder (witnessed ERP: во всех 18 файлах, где
    // есть и CommandsOrder, и SubsystemsOrder, порядок именно такой).
    if !ci.subsystems_order.is_empty() {
        s.push_str("\t<SubsystemsOrder>\r\n");
        for sub in &ci.subsystems_order {
            s.push_str("\t\t<Subsystem>");
            s.push_str(sub);
            s.push_str("</Subsystem>\r\n");
        }
        s.push_str("\t</SubsystemsOrder>\r\n");
    }
    // Designer-only `GroupsOrder` — the group sequence of `order` (derived; EDT omits it). Emitted
    // only when there IS an order region (RE: GroupsOrder always co-occurs with CommandsOrder —
    // 164/164 ERP + SSL).
    if !ci.order.is_empty() {
        s.push_str("\t<GroupsOrder>\r\n");
        for f in &ci.order {
            s.push_str("\t\t<Group>");
            s.push_str(&f.group);
            s.push_str("</Group>\r\n");
        }
        s.push_str("\t</GroupsOrder>\r\n");
    }
    s.push_str("</CommandInterface>");
    s.into_bytes()
}

/// Emit one Designer transposed region (`CommandsPlacement`/`CommandsOrder`) if it carries
/// fragments — one `<Command name>` per command, each with its `<CommandGroup>` (and
/// `<Placement>TEXT</Placement>` when `placement` is set). Tab indent, CRLF — matching the
/// visibility region.
fn emit_designer_region(
    s: &mut String,
    region: &str,
    placement: Option<&str>,
    frags: &[CommandGroupFragment],
) {
    if frags.is_empty() {
        return;
    }
    s.push('\t');
    s.push('<');
    s.push_str(region);
    s.push_str(">\r\n");
    for f in frags {
        for c in &f.commands {
            s.push_str("\t\t<Command name=\"");
            s.push_str(c);
            s.push_str("\">\r\n\t\t\t<CommandGroup>");
            s.push_str(&f.group);
            s.push_str("</CommandGroup>\r\n");
            if let Some(text) = placement {
                s.push_str("\t\t\t<Placement>");
                s.push_str(text);
                s.push_str("</Placement>\r\n");
            }
            s.push_str("\t\t</Command>\r\n");
        }
    }
    s.push_str("\t</");
    s.push_str(region);
    s.push_str(">\r\n");
}

/// `(sidecar path)` beside the descriptor, per format layout (see module docs). `None` for
/// formats without a file-per-object sidecar (cf is a container).
fn cmi_sidecar(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sibling `<obj-dir>/CommandInterface.cmi`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(obj_dir.join(EDT_CMI_FILE))
        }
        // Designer: descriptor `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/CommandInterface.xml`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext").join(DESIGNER_CMI_FILE))
        }
        Format::Cf => None,
    }
}

/// Parse the command-interface sidecar bytes into the canonical [`CommandInterface`] IR,
/// dispatching on the format dialect (подсистемный сайдкар — Designer-`<Placement>` `Auto`).
/// §1.0: any shape outside the witnessed `commandsVisibility` region → a typed error (never
/// a silent skip).
fn parse_command_interface(format: Format, bytes: &[u8]) -> Result<CommandInterface, String> {
    parse_command_interface_with_placement(format, bytes, PLACEMENT_AUTO)
}

/// [`parse_command_interface`] с ЯВНЫМ ожидаемым Designer-`<Placement>`-текстом —
/// конфиг-уровневые cmi-сайдкары (`crate::config_interface_read`) несут `Manual`.
/// У EDT-диалекта placement-текста нет вовсе (контекстно-выводим) — параметр его не меняет.
pub(crate) fn parse_command_interface_with_placement(
    format: Format,
    bytes: &[u8],
    placement_text: &str,
) -> Result<CommandInterface, String> {
    let doc = formats_xml::parse(bytes).map_err(|e| format!("CommandInterface XML: {e}"))?;
    let root = &doc.root;
    if root.local != "CommandInterface" {
        return Err(format!(
            "CommandInterface root element is <{}>, expected <CommandInterface> (§1.0)",
            root.local
        ));
    }
    match format {
        Format::Edt => parse_edt(root),
        Format::Designer => parse_designer(root, placement_text),
        Format::Cf => Err("cf has no CommandInterface sidecar".into()),
    }
}

/// EDT dialect: `<cmi:CommandInterface><commandsVisibility><visibilityFragments>
/// <command>NAME</command><visible/|<visible>[<common>true</common>][<for>…</for>×]</visible>>
/// </visibilityFragments>…</commandsVisibility>[<commandsPlacement>…][<commandsOrder>…]
/// [<subsystemsOrder><subsystems>PATH</subsystems>×]</cmi:CommandInterface>`.
fn parse_edt(root: &Element) -> Result<CommandInterface, String> {
    let mut ci = CommandInterface {
        commands: Vec::new(),
        subsystems_visibility: Vec::new(),
        placement: Vec::new(),
        order: Vec::new(),
        subsystems_order: Vec::new(),
    };
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for region in &root.children {
        if !seen.insert(region.local.as_str()) {
            return Err(format!(
                "CommandInterface carries two <{}> regions (§1.0)",
                region.local
            ));
        }
        match region.local.as_str() {
            "commandsVisibility" => ci.commands = parse_edt_visibility(region)?,
            "subsystemsVisibility" => {
                ci.subsystems_visibility = parse_subsystem_visibility(region, Format::Edt)?
            }
            "commandsPlacement" => {
                ci.placement = parse_edt_fragments(region, "placementFragments")?
            }
            "commandsOrder" => ci.order = parse_edt_fragments(region, "orderFragments")?,
            "subsystemsOrder" => ci.subsystems_order = parse_subsystem_list(region, "subsystems")?,
            other => {
                return Err(format!(
                    "CommandInterface carries unmodelled region <{other}> (only \
                     <commandsVisibility>/<commandsPlacement>/<commandsOrder>/<subsystemsOrder> \
                     are reproducible in the cf <uuid>.1 body, §1.0 — no silent skip)"
                ));
            }
        }
    }
    Ok(ci)
}

/// `subsystemsOrder`/`SubsystemsOrder` region → the ordered child-subsystem path list
/// (`Subsystem.<Родитель>.Subsystem.<Дочерняя>`). `item` — the per-dialect element name
/// (`subsystems` EDT / `Subsystem` Designer). §1.0: any other child / empty text / an empty
/// region errors loudly (witnessed regions are always non-empty).
fn parse_subsystem_list(region: &Element, item: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for ch in &region.children {
        if ch.local != item {
            return Err(format!(
                "{} carries unexpected <{}> (only <{item}> modelled, §1.0)",
                region.local, ch.local
            ));
        }
        if ch.text.is_empty() {
            return Err(format!("{} <{item}> has empty text (§1.0)", region.local));
        }
        out.push(ch.text.clone());
    }
    if out.is_empty() {
        return Err(format!(
            "{} region is empty (§1.0 — unwitnessed shape: every witnessed region is non-empty)",
            region.local
        ));
    }
    Ok(out)
}

/// EDT `<commandsVisibility>` → `Vec<CommandVisibility>` (`<visibilityFragments>` per command).
fn parse_edt_visibility(region: &Element) -> Result<Vec<CommandVisibility>, String> {
    let mut commands = Vec::new();
    for frag in &region.children {
        if frag.local != "visibilityFragments" {
            return Err(format!(
                "commandsVisibility carries unexpected <{}> (only <visibilityFragments> \
                 modelled, §1.0)",
                frag.local
            ));
        }
        let command = child_text(frag, "command")?;
        let (common_visible, role_values) = parse_edt_visible(frag)?;
        commands.push(CommandVisibility {
            command,
            common_visible,
            role_values,
        });
    }
    Ok(commands)
}

// Subsystem visibility is a typed ordered list; every source node and attribute
// is consumed. It shares the existing visibility model, not command identity.
fn visibility_node(
    el: &Element,
    name: &str,
    prefix: &str,
    attrs: &[&str],
    leaf: bool,
) -> Result<(), String> {
    if el.local != name
        || el.prefix != prefix
        || el.attrs.len() != attrs.len()
        || el.attrs.iter().any(|a| !attrs.contains(&a.name.as_str()))
        || (leaf && !el.children.is_empty())
        || (!leaf && !el.text.is_empty())
    {
        return Err(format!(
            "subsystems visibility: unconsumed/ambiguous <{name}> shape"
        ));
    }
    Ok(())
}

fn parse_subsystem_visibility(
    region: &Element,
    format: Format,
) -> Result<Vec<SubsystemVisibility>, String> {
    let mut out = Vec::new();
    let (region_name, item_name) = match format {
        Format::Edt => ("subsystemsVisibility", "visibilityFragments"),
        Format::Designer => ("SubsystemsVisibility", "Subsystem"),
        Format::Cf => return Err("cf has no subsystem visibility XML".into()),
    };
    visibility_node(region, region_name, "", &[], false)?;
    for item in &region.children {
        let (subsystem, common_visible, role_values) = match format {
            Format::Edt => {
                visibility_node(item, item_name, "", &[], false)?;
                if item.children.len() != 2
                    || item.children[0].local != "subsystem"
                    || item.children[1].local != "visible"
                {
                    return Err(
                        "subsystem visibility fragment requires subsystem followed by visible"
                            .into(),
                    );
                }
                let reference = &item.children[0];
                visibility_node(reference, "subsystem", "", &[], true)?;
                let visible = &item.children[1];
                visibility_node(visible, "visible", "", &[], false)?;
                let mut common = false;
                let mut common_seen = false;
                let mut roles = Vec::new();
                for child in &visible.children {
                    match child.local.as_str() {
                        "common" if !common_seen && roles.is_empty() => {
                            visibility_node(child, "common", "", &[], true)?;
                            common = parse_bool(&child.text, "common")?;
                            common_seen = true;
                        }
                        "for" => {
                            visibility_node(child, "for", "", &[], false)?;
                            if !matches!(child.children.len(), 1 | 2) {
                                return Err("subsystem visibility role requires one role and optional value".into());
                            }
                            for field in &child.children {
                                if !matches!(field.local.as_str(), "role" | "value") {
                                    return Err("unknown subsystem role field".into());
                                }
                                visibility_node(field, &field.local, "", &[], true)?;
                            }
                            roles.push(parse_edt_for(child)?);
                        }
                        _ => {
                            return Err(
                                "unknown/duplicate/out-of-order subsystem visibility field".into()
                            );
                        }
                    }
                }
                (reference.text.clone(), common, roles)
            }
            Format::Designer => {
                visibility_node(item, item_name, "", &["name"], false)?;
                if item.children.len() != 1 {
                    return Err("Subsystem requires exactly one Visibility".into());
                }
                let visibility = &item.children[0];
                visibility_node(visibility, "Visibility", "", &[], false)?;
                let mut common = None;
                let mut roles = Vec::new();
                for field in &visibility.children {
                    match field.local.as_str() {
                        "Common" if common.is_none() && roles.is_empty() => {
                            visibility_node(field, "Common", "xr", &[], true)?;
                            common = Some(parse_bool(&field.text, "Common")?);
                        }
                        "Value" if common.is_some() => {
                            visibility_node(field, "Value", "xr", &["name"], true)?;
                            roles.push(RoleVisibility {
                                role: field.attrs[0].value.clone(),
                                visible: parse_bool(&field.text, "Value")?,
                            });
                        }
                        _ => {
                            return Err(
                                "unknown/duplicate/out-of-order native subsystem visibility field"
                                    .into(),
                            );
                        }
                    }
                }
                (
                    item.attrs[0].value.clone(),
                    common.ok_or("subsystem Visibility lacks Common")?,
                    roles,
                )
            }
            Format::Cf => unreachable!(),
        };
        if !subsystem.starts_with("Subsystem.")
            || subsystem.len() == "Subsystem.".len()
            || role_values
                .iter()
                .any(|r| !r.role.starts_with("Role.") || r.role.len() == "Role.".len())
        {
            return Err(
                "subsystem visibility requires complete Subsystem and Role references".into(),
            );
        }
        out.push(SubsystemVisibility {
            subsystem,
            common_visible,
            role_values,
        });
    }
    Ok(out)
}

fn visibility_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn emit_subsystem_visibility_edt(s: &mut String, rows: &[SubsystemVisibility]) {
    if rows.is_empty() {
        return;
    }
    s.push_str("  <subsystemsVisibility>\r\n");
    for row in rows {
        s.push_str("    <visibilityFragments>\r\n      <subsystem>");
        s.push_str(&visibility_escape(&row.subsystem));
        s.push_str("</subsystem>\r\n");
        if !row.common_visible && row.role_values.is_empty() {
            s.push_str("      <visible/>\r\n");
        } else {
            s.push_str("      <visible>\r\n");
            if row.common_visible {
                s.push_str("        <common>true</common>\r\n");
            }
            for role in &row.role_values {
                s.push_str("        <for>\r\n");
                if role.visible {
                    s.push_str("          <value>true</value>\r\n");
                }
                s.push_str("          <role>");
                s.push_str(&visibility_escape(&role.role));
                s.push_str("</role>\r\n        </for>\r\n");
            }
            s.push_str("      </visible>\r\n");
        }
        s.push_str("    </visibilityFragments>\r\n");
    }
    s.push_str("  </subsystemsVisibility>\r\n");
}

fn emit_subsystem_visibility_designer(s: &mut String, rows: &[SubsystemVisibility]) {
    if rows.is_empty() {
        return;
    }
    s.push_str("\t<SubsystemsVisibility>\r\n");
    for row in rows {
        s.push_str("\t\t<Subsystem name=\"");
        s.push_str(&visibility_escape(&row.subsystem));
        s.push_str("\">\r\n\t\t\t<Visibility>\r\n\t\t\t\t<xr:Common>");
        s.push_str(if row.common_visible { "true" } else { "false" });
        s.push_str("</xr:Common>\r\n");
        for role in &row.role_values {
            s.push_str("\t\t\t\t<xr:Value name=\"");
            s.push_str(&visibility_escape(&role.role));
            s.push_str("\">");
            s.push_str(if role.visible { "true" } else { "false" });
            s.push_str("</xr:Value>\r\n");
        }
        s.push_str("\t\t\t</Visibility>\r\n\t\t</Subsystem>\r\n");
    }
    s.push_str("\t</SubsystemsVisibility>\r\n");
}

/// EDT grouped region (`commandsPlacement`/`commandsOrder`) → fragments. Each `frag_name`
/// (`placementFragments`/`orderFragments`) carries exactly one `<group>` then its `<commands>*`.
/// §1.0: any other child, a missing/empty group, or an empty command errors loudly.
fn parse_edt_fragments(
    region: &Element,
    frag_name: &str,
) -> Result<Vec<CommandGroupFragment>, String> {
    let mut out = Vec::new();
    for frag in &region.children {
        if frag.local != frag_name {
            return Err(format!(
                "{} carries unexpected <{}> (only <{frag_name}> modelled, §1.0)",
                region.local, frag.local
            ));
        }
        let mut group: Option<String> = None;
        let mut commands: Vec<String> = Vec::new();
        for ch in &frag.children {
            match ch.local.as_str() {
                "group" => {
                    if group.is_some() {
                        return Err(format!("<{frag_name}> carries two <group> (§1.0)"));
                    }
                    if ch.text.is_empty() {
                        return Err(format!("<{frag_name}> <group> has empty text (§1.0)"));
                    }
                    group = Some(ch.text.clone());
                }
                "commands" => {
                    if ch.text.is_empty() {
                        return Err(format!("<{frag_name}> <commands> has empty text (§1.0)"));
                    }
                    commands.push(ch.text.clone());
                }
                other => return Err(format!("<{frag_name}> carries unexpected <{other}> (§1.0)")),
            }
        }
        let group = group.ok_or_else(|| format!("<{frag_name}> has no <group> (§1.0)"))?;
        out.push(CommandGroupFragment { group, commands });
    }
    Ok(out)
}

/// EDT `<visible>` visibility → `(common, role_values)`. Witnessed shapes (SSL + ERP census):
/// empty `<visible/>` == Common false, no roles; `<common>BOOL</common>` FIRST (optional —
/// omission == false); then `<for>`× per role: `<value>true</value>` (optional — omission ==
/// false; `"true"` — единственный witnessed текст) + `<role>Role.X</role>`. §1.0: any other
/// child / order (`<common>` after a `<for>`) → error.
fn parse_edt_visible(frag: &Element) -> Result<(bool, Vec<RoleVisibility>), String> {
    let visible = frag
        .child("visible")
        .ok_or_else(|| "visibilityFragments has no <visible> element (§1.0)".to_string())?;
    let mut common = false;
    let mut roles: Vec<RoleVisibility> = Vec::new();
    for ch in &visible.children {
        match ch.local.as_str() {
            "common" => {
                if !roles.is_empty() {
                    return Err(
                        "<visible> carries <common> AFTER a <for> (§1.0 — unwitnessed order)"
                            .to_string(),
                    );
                }
                common = parse_bool(&ch.text, "common")?;
            }
            "for" => roles.push(parse_edt_for(ch)?),
            other => {
                return Err(format!(
                    "<visible> carries unmodelled child <{other}> (only <common>/<for> \
                     witnessed, §1.0)"
                ));
            }
        }
    }
    Ok((common, roles))
}

/// One EDT `<for>` role-visibility entry: `[<value>true</value>]<role>Role.X</role>`.
/// §1.0: `<value>` с текстом ≠ `"true"`, второй `<value>`/`<role>`, иной ребёнок или
/// отсутствующий `<role>` — отказ (незасвидетельствованный шейп).
fn parse_edt_for(el: &Element) -> Result<RoleVisibility, String> {
    let mut value = false;
    let mut value_seen = false;
    let mut role: Option<String> = None;
    for ch in &el.children {
        match ch.local.as_str() {
            "value" => {
                if value_seen {
                    return Err("<for> carries two <value> (§1.0)".to_string());
                }
                if role.is_some() {
                    return Err(
                        "<for> carries <value> AFTER <role> (§1.0 — unwitnessed order)".to_string(),
                    );
                }
                if ch.text != "true" {
                    return Err(format!(
                        "<for> <value> is {:?} — only \"true\" witnessed (omission == false), \
                         §1.0",
                        ch.text
                    ));
                }
                value = true;
                value_seen = true;
            }
            "role" => {
                if role.is_some() {
                    return Err("<for> carries two <role> (§1.0)".to_string());
                }
                if ch.text.is_empty() {
                    return Err("<for> <role> has empty text (§1.0)".to_string());
                }
                role = Some(ch.text.clone());
            }
            other => return Err(format!("<for> carries unexpected <{other}> (§1.0)")),
        }
    }
    let role = role.ok_or_else(|| "<for> has no <role> (§1.0)".to_string())?;
    Ok(RoleVisibility {
        role,
        visible: value,
    })
}

/// Designer dialect: `<CommandInterface version="…"><CommandsVisibility><Command name="NAME">
/// <Visibility><xr:Common>BOOL</xr:Common>[<xr:Value name="Role.X">BOOL</xr:Value>×]
/// </Visibility></Command>…</CommandsVisibility>[<CommandsPlacement>…][<CommandsOrder>…]
/// [<SubsystemsOrder>…][<GroupsOrder>…]</CommandInterface>`. §1.0: отсутствующая /
/// невитнессированная версия корня — отказ (иначе она была бы молча перештампована таргетом).
fn parse_designer(root: &Element, placement_text: &str) -> Result<CommandInterface, String> {
    let version = root
        .attr("version")
        .ok_or_else(|| "CommandInterface root has no version attribute (§1.0)".to_string())?;
    crate::sidecar_version::parse_witnessed(&version.value, "CommandInterface")?;
    let mut ci = CommandInterface {
        commands: Vec::new(),
        subsystems_visibility: Vec::new(),
        placement: Vec::new(),
        order: Vec::new(),
        subsystems_order: Vec::new(),
    };
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for region in &root.children {
        if !seen.insert(region.local.as_str()) {
            return Err(format!(
                "CommandInterface carries two <{}> regions (§1.0)",
                region.local
            ));
        }
        match region.local.as_str() {
            "CommandsVisibility" => ci.commands = parse_designer_visibility(region)?,
            "SubsystemsVisibility" => {
                ci.subsystems_visibility = parse_subsystem_visibility(region, Format::Designer)?
            }
            "CommandsPlacement" => {
                ci.placement = parse_designer_fragments(region, Some(placement_text))?
            }
            "CommandsOrder" => ci.order = parse_designer_fragments(region, None)?,
            "SubsystemsOrder" => ci.subsystems_order = parse_subsystem_list(region, "Subsystem")?,
            // `GroupsOrder` is a Designer-ONLY region: the order of command GROUPS. RE (SSL:
            // Администрирование/ОценкаПроизводительности/УчетОригиналов… + ERP 164/164) shows it
            // is EXACTLY the group sequence of `CommandsOrder` (EDT has no such region — it is
            // redundant with the order fragments). We DERIVE it from `ci.order` on write; here we
            // VERIFY the source agrees (§1.0 — a divergence would break the derive-on-write
            // assumption, so error). Region order guarantees `CommandsOrder` was parsed first.
            "GroupsOrder" => verify_groups_order(region, &ci.order)?,
            other => {
                return Err(format!(
                    "CommandInterface carries unmodelled region <{other}> (only \
                     <CommandsVisibility>/<CommandsPlacement>/<CommandsOrder>/<SubsystemsOrder>/\
                     <GroupsOrder> are reproducible in the cf <uuid>.1 body, §1.0 — no silent \
                     skip)"
                ));
            }
        }
    }
    Ok(ci)
}

/// §1.0 verify the Designer `<GroupsOrder>` region equals the group sequence of `order` (the
/// invariant that lets us DERIVE GroupsOrder from `order` on write, so EDT — which omits the
/// region — round-trips to designer byte-exact). A `<Group>` list not matching order's fragment
/// groups (in order) errors loudly rather than silently accept an unreconstructable ordering.
fn verify_groups_order(region: &Element, order: &[CommandGroupFragment]) -> Result<(), String> {
    let mut groups: Vec<&str> = Vec::new();
    for g in &region.children {
        if g.local != "Group" {
            return Err(format!(
                "GroupsOrder carries unexpected <{}> (only <Group> modelled, §1.0)",
                g.local
            ));
        }
        if g.text.is_empty() {
            return Err("GroupsOrder <Group> has empty text (§1.0)".to_string());
        }
        groups.push(&g.text);
    }
    let from_order: Vec<&str> = order.iter().map(|f| f.group.as_str()).collect();
    if groups != from_order {
        return Err(format!(
            "GroupsOrder {groups:?} != commandsOrder group sequence {from_order:?} — the derive-\
             from-order invariant does not hold (§1.0 — GroupsOrder would be lost on edt round-trip)"
        ));
    }
    Ok(())
}

/// Designer `<CommandsVisibility>` → `Vec<CommandVisibility>` (`<Command name>` per command).
/// `<Visibility>` children are parsed STRICTLY in witnessed order: `<xr:Common>` FIRST
/// (обязателен — 1965/1965), then `<xr:Value name="Role.X">BOOL</xr:Value>`× (§1.0: любой иной
/// ребёнок/порядок — отказ; раньше `<xr:Value>` МОЛЧА ТЕРЯЛСЯ — silent-drop дыра).
fn parse_designer_visibility(region: &Element) -> Result<Vec<CommandVisibility>, String> {
    let mut commands = Vec::new();
    for cmd in &region.children {
        if cmd.local != "Command" {
            return Err(format!(
                "CommandsVisibility carries unexpected <{}> (only <Command> modelled, §1.0)",
                cmd.local
            ));
        }
        let command = cmd
            .attr("name")
            .map(|a| a.value.clone())
            .ok_or_else(|| "<Command> has no name attribute (§1.0)".to_string())?;
        let visibility = cmd
            .child("Visibility")
            .ok_or_else(|| "<Command> has no <Visibility> element (§1.0)".to_string())?;
        let mut common: Option<bool> = None;
        let mut role_values: Vec<RoleVisibility> = Vec::new();
        for ch in &visibility.children {
            match ch.local.as_str() {
                "Common" => {
                    if common.is_some() {
                        return Err("<Visibility> carries two <Common> (§1.0)".to_string());
                    }
                    if !role_values.is_empty() {
                        return Err("<Visibility> carries <Common> AFTER an <xr:Value> (§1.0 — \
                                    unwitnessed order)"
                            .to_string());
                    }
                    common = Some(parse_bool(&ch.text, "Common")?);
                }
                "Value" => {
                    let role = ch
                        .attr("name")
                        .map(|a| a.value.clone())
                        .ok_or_else(|| "<xr:Value> has no name attribute (§1.0)".to_string())?;
                    if role.is_empty() {
                        return Err("<xr:Value> name attribute is empty (§1.0)".to_string());
                    }
                    let visible = parse_bool(&ch.text, "Value")?;
                    role_values.push(RoleVisibility { role, visible });
                }
                other => {
                    return Err(format!(
                        "<Visibility> carries unmodelled child <{other}> (only <xr:Common>/\
                         <xr:Value> witnessed, §1.0)"
                    ));
                }
            }
        }
        let common_visible =
            common.ok_or_else(|| "<Visibility> has no <Common> element (§1.0)".to_string())?;
        commands.push(CommandVisibility {
            command,
            common_visible,
            role_values,
        });
    }
    Ok(commands)
}

/// Designer transposed region (`CommandsPlacement`/`CommandsOrder`) → fragments. Each `<Command
/// name>` carries a `<CommandGroup>` (and, when `placement` is set, a `<Placement>` that MUST
/// carry ровно этот текст — witnessed `Auto` у подсистем / `Manual` у конфиг-уровня;
/// §1.0-guarded). Per-command entries are grouped back into EDT-shaped fragments by consecutive
/// `<CommandGroup>` runs; a group reappearing non-contiguously cannot be reconstructed into
/// distinct EDT fragments and errors loudly (§1.0 — no silent reorder).
fn parse_designer_fragments(
    region: &Element,
    placement: Option<&str>,
) -> Result<Vec<CommandGroupFragment>, String> {
    let mut items: Vec<(String, String)> = Vec::new();
    for cmd in &region.children {
        if cmd.local != "Command" {
            return Err(format!(
                "{} carries unexpected <{}> (only <Command> modelled, §1.0)",
                region.local, cmd.local
            ));
        }
        let name = cmd
            .attr("name")
            .map(|a| a.value.clone())
            .ok_or_else(|| "<Command> has no name attribute (§1.0)".to_string())?;
        let mut group: Option<String> = None;
        let mut has_placement = false;
        for ch in &cmd.children {
            match ch.local.as_str() {
                "CommandGroup" => {
                    if ch.text.is_empty() {
                        return Err("<CommandGroup> has empty text (§1.0)".to_string());
                    }
                    group = Some(ch.text.clone());
                }
                "Placement" if placement.is_some() => {
                    let expected = placement.expect("checked is_some");
                    if ch.text != expected {
                        return Err(format!(
                            "<Placement> is {:?}, only {expected:?} witnessed for this \
                             sidecar kind (§1.0)",
                            ch.text
                        ));
                    }
                    has_placement = true;
                }
                other => {
                    return Err(format!(
                        "{} <Command> carries unexpected <{other}> (§1.0)",
                        region.local
                    ));
                }
            }
        }
        if placement.is_some() && !has_placement {
            return Err(
                "CommandsPlacement <Command> has no <Placement> element (§1.0)".to_string(),
            );
        }
        let group =
            group.ok_or_else(|| "<Command> has no <CommandGroup> element (§1.0)".to_string())?;
        items.push((name, group));
    }
    group_into_fragments(items)
}

/// Group an ordered per-command `(command, group)` list into EDT-shaped fragments by consecutive
/// group runs. §1.0: a group reappearing after a different group intervened cannot map back to
/// distinct EDT fragments unambiguously → error (never a silent merge/reorder).
fn group_into_fragments(items: Vec<(String, String)>) -> Result<Vec<CommandGroupFragment>, String> {
    let mut out: Vec<CommandGroupFragment> = Vec::new();
    let mut closed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (command, group) in items {
        match out.last_mut() {
            Some(frag) if frag.group == group => frag.commands.push(command),
            _ => {
                if closed.contains(&group) {
                    return Err(format!(
                        "CommandInterface group {group:?} appears non-contiguously — cannot \
                         reconstruct EDT fragments (§1.0 — no silent reorder)"
                    ));
                }
                if let Some(prev) = out.last() {
                    closed.insert(prev.group.clone());
                }
                out.push(CommandGroupFragment {
                    group,
                    commands: vec![command],
                });
            }
        }
    }
    Ok(out)
}

/// The direct text of the first `<local>` child (§1.0: required child, non-empty).
fn child_text(parent: &Element, local: &str) -> Result<String, String> {
    let el = parent
        .child(local)
        .ok_or_else(|| format!("missing <{local}> element (§1.0)"))?;
    if el.text.is_empty() {
        return Err(format!("<{local}> has empty text (§1.0)"));
    }
    Ok(el.text.clone())
}

/// Strict boolean: `"true"`/`"false"` only (§1.0 — no lenient parse).
fn parse_bool(text: &str, ctx: &str) -> Result<bool, String> {
    match text {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!(
            "<{ctx}> is {other:?}, expected \"true\"/\"false\" (§1.0)"
        )),
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    const EDT_CMI: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<cmi:CommandInterface xmlns:cmi="http://g5.1c.ru/v8/dt/cmi">
  <commandsVisibility>
    <visibilityFragments>
      <command>Catalog.X.StandardCommand.OpenList</command>
      <visible/>
    </visibilityFragments>
    <visibilityFragments>
      <command>Catalog.X.StandardCommand.Create</command>
      <visible>
        <common>true</common>
      </visible>
    </visibilityFragments>
  </commandsVisibility>
</cmi:CommandInterface>
"#;

    const DESIGNER_CMI: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.21">
	<CommandsVisibility>
		<Command name="Catalog.X.StandardCommand.OpenList">
			<Visibility>
				<xr:Common>false</xr:Common>
			</Visibility>
		</Command>
		<Command name="Catalog.X.StandardCommand.Create">
			<Visibility>
				<xr:Common>true</xr:Common>
			</Visibility>
		</Command>
	</CommandsVisibility>
</CommandInterface>"#;

    #[test]
    fn edt_sidecar_path_is_sibling() {
        let p = Path::new("/root/Subsystems/Подсистема/Подсистема.mdo");
        let s = cmi_sidecar(Format::Edt, p).unwrap();
        assert!(
            s.ends_with(Path::new("Subsystems/Подсистема/CommandInterface.cmi")),
            "got {s:?}"
        );
    }

    #[test]
    fn designer_sidecar_path_is_ext_subdir() {
        let p = Path::new("/root/Subsystems/Подсистема.xml");
        let s = cmi_sidecar(Format::Designer, p).unwrap();
        assert!(
            s.ends_with(Path::new("Subsystems/Подсистема/Ext/CommandInterface.xml")),
            "got {s:?}"
        );
    }

    #[test]
    fn edt_and_designer_parse_to_equal_ir() {
        // §1.6: both dialects yield the SAME canonical CommandInterface (X by construction).
        let edt = parse_command_interface(Format::Edt, EDT_CMI).unwrap();
        let des = parse_command_interface(Format::Designer, DESIGNER_CMI).unwrap();
        assert_eq!(edt, des);
        assert_eq!(edt.commands.len(), 2);
        assert_eq!(
            edt.commands[0].command,
            "Catalog.X.StandardCommand.OpenList"
        );
        assert!(!edt.commands[0].common_visible, "empty <visible/> == false");
        assert_eq!(edt.commands[1].command, "Catalog.X.StandardCommand.Create");
        assert!(
            edt.commands[1].common_visible,
            "<common>true</common> == true"
        );
    }

    #[test]
    fn non_subsystem_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([0; 16]));
        attach_command_interface(
            Format::Edt,
            "Constant",
            Path::new("/nope/C/C.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.command_interface.is_none());
    }

    #[test]
    fn subsystem_without_sidecar_is_noop() {
        // §1.0 OPTIONAL: absent sidecar ⇒ no-op (not a hard error — unlike Role rights).
        let mut obj =
            MetadataObject::new(ObjectKind::new("Subsystem"), "Подсистема", Uuid([0; 16]));
        attach_command_interface(
            Format::Edt,
            "Subsystem",
            Path::new("/nonexistent-root/Subsystems/Подсистема/Подсистема.mdo"),
            &mut obj,
        )
        .expect("a subsystem without a command-interface sidecar must be a no-op");
        assert!(obj.command_interface.is_none());
    }

    #[test]
    fn unmodelled_region_errors() {
        // §1.0: a region outside the four witnessed ones is refused (not a silent skip).
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<cmi:CommandInterface xmlns:cmi="http://g5.1c.ru/v8/dt/cmi">
  <commandsHidden>
    <command>Catalog.X.StandardCommand.OpenList</command>
  </commandsHidden>
</cmi:CommandInterface>
"#;
        let err = parse_command_interface(Format::Edt, xml).unwrap_err();
        assert!(err.contains("unmodelled region"), "got {err}");
    }

    /// ERP-витнесс (выжимка `CRMИМаркетинг`, оба диалекта): по-ролевые значения видимости +
    /// порядок подсистем. §1.6: оба диалекта дают РАВНЫЙ канон; сериализация byte-exact в оба.
    #[test]
    fn erp_role_values_and_subsystems_order_roundtrip() {
        const EDT_ERP: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<cmi:CommandInterface xmlns:cmi=\"http://g5.1c.ru/v8/dt/cmi\">\r\n",
            "  <commandsVisibility>\r\n",
            "    <visibilityFragments>\r\n",
            "      <command>CommonCommand.ПанельОтчетовCRMИМаркетинг</command>\r\n",
            "      <visible>\r\n",
            "        <common>true</common>\r\n",
            "        <for>\r\n",
            "          <value>true</value>\r\n",
            "          <role>Role.ОтчетыМаркетолога</role>\r\n",
            "        </for>\r\n",
            "        <for>\r\n",
            "          <role>Role.АдминистраторСистемы</role>\r\n",
            "        </for>\r\n",
            "      </visible>\r\n",
            "    </visibilityFragments>\r\n",
            "  </commandsVisibility>\r\n",
            "  <subsystemsOrder>\r\n",
            "    <subsystems>Subsystem.CRMИМаркетинг.Subsystem.НастройкиИСправочники</subsystems>\r\n",
            "    <subsystems>Subsystem.CRMИМаркетинг.Subsystem.НСИПродаж</subsystems>\r\n",
            "  </subsystemsOrder>\r\n",
            "</cmi:CommandInterface>\r\n",
        );
        const DESIGNER_ERP: &str = concat!(
            "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<CommandInterface xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n",
            "\t<CommandsVisibility>\r\n",
            "\t\t<Command name=\"CommonCommand.ПанельОтчетовCRMИМаркетинг\">\r\n",
            "\t\t\t<Visibility>\r\n",
            "\t\t\t\t<xr:Common>true</xr:Common>\r\n",
            "\t\t\t\t<xr:Value name=\"Role.ОтчетыМаркетолога\">true</xr:Value>\r\n",
            "\t\t\t\t<xr:Value name=\"Role.АдминистраторСистемы\">false</xr:Value>\r\n",
            "\t\t\t</Visibility>\r\n",
            "\t\t</Command>\r\n",
            "\t</CommandsVisibility>\r\n",
            "\t<SubsystemsOrder>\r\n",
            "\t\t<Subsystem>Subsystem.CRMИМаркетинг.Subsystem.НастройкиИСправочники</Subsystem>\r\n",
            "\t\t<Subsystem>Subsystem.CRMИМаркетинг.Subsystem.НСИПродаж</Subsystem>\r\n",
            "\t</SubsystemsOrder>\r\n",
            "</CommandInterface>",
        );
        let e = parse_command_interface(Format::Edt, EDT_ERP.as_bytes()).unwrap();
        let d = parse_command_interface(Format::Designer, DESIGNER_ERP.as_bytes()).unwrap();
        assert_eq!(e, d, "edt canon == designer canon (§1.6)");
        assert_eq!(e.commands.len(), 1);
        assert!(e.commands[0].common_visible);
        assert_eq!(
            e.commands[0].role_values,
            vec![
                RoleVisibility {
                    role: "Role.ОтчетыМаркетолога".into(),
                    visible: true
                },
                RoleVisibility {
                    role: "Role.АдминистраторСистемы".into(),
                    visible: false
                },
            ]
        );
        assert_eq!(e.subsystems_order.len(), 2);
        assert_eq!(
            serialize_edt(&e),
            EDT_ERP.as_bytes(),
            "EDT byte-exact incl. <for> role values + subsystemsOrder"
        );
        assert_eq!(
            serialize_designer(&d, morph1c_core::version::ERP),
            DESIGNER_ERP.as_bytes(),
            "Designer byte-exact under the 2.20 source version"
        );
    }

    /// ERP: сайдкар из ОДНОГО региона `subsystemsOrder` (witnessed 4 файла) — регион видимости
    /// НЕ эмитится, когда команд нет.
    #[test]
    fn subsystems_order_only_sidecar_roundtrips() {
        const EDT_ONLY: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<cmi:CommandInterface xmlns:cmi=\"http://g5.1c.ru/v8/dt/cmi\">\r\n",
            "  <subsystemsOrder>\r\n",
            "    <subsystems>Subsystem.Продажи.Subsystem.ОптовыеПродажи</subsystems>\r\n",
            "  </subsystemsOrder>\r\n",
            "</cmi:CommandInterface>\r\n",
        );
        let ci = parse_command_interface(Format::Edt, EDT_ONLY.as_bytes()).unwrap();
        assert!(ci.commands.is_empty());
        assert_eq!(ci.subsystems_order.len(), 1);
        assert_eq!(
            serialize_edt(&ci),
            EDT_ONLY.as_bytes(),
            "no empty <commandsVisibility> fabricated"
        );
        let d = serialize_designer(&ci, morph1c_core::version::ERP);
        let text = String::from_utf8(d.clone()).unwrap();
        assert!(
            !text.contains("<CommandsVisibility>"),
            "no empty designer visibility region: {text}"
        );
        assert_eq!(
            parse_command_interface(Format::Designer, &d).unwrap(),
            ci,
            "designer projection re-reads to the same canon (§1.6)"
        );
    }

    /// §1.0: Designer-сайдкар с невитнессированной версией корня → отказ.
    #[test]
    fn unwitnessed_designer_version_is_refused() {
        let bad = String::from_utf8_lossy(DESIGNER_CMI).replace("2.21", "2.19");
        let err = parse_command_interface(Format::Designer, bad.as_bytes()).unwrap_err();
        assert!(err.contains("not a witnessed"), "got {err}");
    }

    #[test]
    fn non_bool_common_errors() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<cmi:CommandInterface xmlns:cmi="http://g5.1c.ru/v8/dt/cmi">
  <commandsVisibility>
    <visibilityFragments>
      <command>Catalog.X.StandardCommand.OpenList</command>
      <visible><common>maybe</common></visible>
    </visibilityFragments>
  </commandsVisibility>
</cmi:CommandInterface>
"#;
        let err = parse_command_interface(Format::Edt, xml).unwrap_err();
        assert!(err.contains("expected \"true\"/\"false\""), "got {err}");
    }

    // EDT .cmi carrying all three regions (visibility + placement + order), SSL-shaped
    // (Администрирование/ОценкаПроизводительности): 2-space indent, CRLF, trailing newline.
    // `concat!` (NOT `\`-continuation, which would strip the leading indentation).
    const EDT_CMI_REGIONS: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<cmi:CommandInterface xmlns:cmi=\"http://g5.1c.ru/v8/dt/cmi\">\r\n",
        "  <commandsVisibility>\r\n",
        "    <visibilityFragments>\r\n",
        "      <command>Catalog.X.StandardCommand.OpenList</command>\r\n",
        "      <visible/>\r\n",
        "    </visibilityFragments>\r\n",
        "  </commandsVisibility>\r\n",
        "  <commandsPlacement>\r\n",
        "    <placementFragments>\r\n",
        "      <group>NavigationPanelImportant</group>\r\n",
        "      <commands>Catalog.X.StandardCommand.OpenList</commands>\r\n",
        "      <commands>Catalog.Y.StandardCommand.OpenList</commands>\r\n",
        "    </placementFragments>\r\n",
        "  </commandsPlacement>\r\n",
        "  <commandsOrder>\r\n",
        "    <orderFragments>\r\n",
        "      <group>NavigationPanelOrdinary</group>\r\n",
        "      <commands>Catalog.Z.StandardCommand.OpenList</commands>\r\n",
        "    </orderFragments>\r\n",
        "  </commandsOrder>\r\n",
        "</cmi:CommandInterface>\r\n",
    );

    // Designer counterpart: BOM, four namespaces + version, tab indent, CRLF, NO trailing newline.
    // CommandsPlacement carries per-command <Placement>Auto</Placement>; GroupsOrder mirrors order.
    const DESIGNER_CMI_REGIONS: &str = concat!(
        "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<CommandInterface xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n",
        "\t<CommandsVisibility>\r\n",
        "\t\t<Command name=\"Catalog.X.StandardCommand.OpenList\">\r\n",
        "\t\t\t<Visibility>\r\n",
        "\t\t\t\t<xr:Common>false</xr:Common>\r\n",
        "\t\t\t</Visibility>\r\n",
        "\t\t</Command>\r\n",
        "\t</CommandsVisibility>\r\n",
        "\t<CommandsPlacement>\r\n",
        "\t\t<Command name=\"Catalog.X.StandardCommand.OpenList\">\r\n",
        "\t\t\t<CommandGroup>NavigationPanelImportant</CommandGroup>\r\n",
        "\t\t\t<Placement>Auto</Placement>\r\n",
        "\t\t</Command>\r\n",
        "\t\t<Command name=\"Catalog.Y.StandardCommand.OpenList\">\r\n",
        "\t\t\t<CommandGroup>NavigationPanelImportant</CommandGroup>\r\n",
        "\t\t\t<Placement>Auto</Placement>\r\n",
        "\t\t</Command>\r\n",
        "\t</CommandsPlacement>\r\n",
        "\t<CommandsOrder>\r\n",
        "\t\t<Command name=\"Catalog.Z.StandardCommand.OpenList\">\r\n",
        "\t\t\t<CommandGroup>NavigationPanelOrdinary</CommandGroup>\r\n",
        "\t\t</Command>\r\n",
        "\t</CommandsOrder>\r\n",
        "\t<GroupsOrder>\r\n",
        "\t\t<Group>NavigationPanelOrdinary</Group>\r\n",
        "\t</GroupsOrder>\r\n",
        "</CommandInterface>",
    );

    #[test]
    fn edt_placement_order_roundtrips_byte_exact() {
        let ci = parse_command_interface(Format::Edt, EDT_CMI_REGIONS.as_bytes()).unwrap();
        assert_eq!(ci.placement.len(), 1, "one placement fragment");
        assert_eq!(ci.placement[0].group, "NavigationPanelImportant");
        assert_eq!(ci.placement[0].commands.len(), 2);
        assert_eq!(ci.order.len(), 1);
        assert_eq!(ci.order[0].group, "NavigationPanelOrdinary");
        assert_eq!(
            serialize_edt(&ci),
            EDT_CMI_REGIONS.as_bytes(),
            "EDT read→write byte-exact"
        );
    }

    #[test]
    fn designer_placement_order_roundtrips_byte_exact() {
        let ci =
            parse_command_interface(Format::Designer, DESIGNER_CMI_REGIONS.as_bytes()).unwrap();
        assert_eq!(ci.placement.len(), 1);
        assert_eq!(
            ci.placement[0].commands,
            vec![
                "Catalog.X.StandardCommand.OpenList",
                "Catalog.Y.StandardCommand.OpenList"
            ]
        );
        assert_eq!(ci.order.len(), 1);
        assert_eq!(
            serialize_designer(&ci, morph1c_core::version::SSL),
            DESIGNER_CMI_REGIONS.as_bytes(),
            "Designer read→write byte-exact"
        );
    }

    #[test]
    fn edt_designer_regions_parse_to_equal_ir() {
        // §1.6: both dialects yield the SAME canonical CommandInterface (placement/order X too).
        let edt = parse_command_interface(Format::Edt, EDT_CMI_REGIONS.as_bytes()).unwrap();
        let des =
            parse_command_interface(Format::Designer, DESIGNER_CMI_REGIONS.as_bytes()).unwrap();
        assert_eq!(
            edt, des,
            "edt and designer regions canonicalize to identical IR"
        );
    }

    #[test]
    fn designer_groups_order_mismatch_errors() {
        // §1.0: a GroupsOrder that does not mirror the order region's group sequence errors.
        let bad = DESIGNER_CMI_REGIONS.replace(
            "<GroupsOrder>\r\n\t\t<Group>NavigationPanelOrdinary</Group>",
            "<GroupsOrder>\r\n\t\t<Group>NavigationPanelImportant</Group>",
        );
        let err = parse_command_interface(Format::Designer, bad.as_bytes()).unwrap_err();
        assert!(err.contains("derive-from-order invariant"), "got {err}");
    }

    #[test]
    fn attaches_edt_sidecar_into_ir() {
        // Self-contained: write a real EDT .cmi to a temp EDT layout, attach it, assert IR.
        let base = std::env::temp_dir().join(format!(
            "morph1c-cmi-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let obj_dir = base.join("Subsystems").join("Подсистема");
        std::fs::create_dir_all(&obj_dir).unwrap();
        crate::fsio::write(obj_dir.join("CommandInterface.cmi"), EDT_CMI).unwrap();
        let descriptor = obj_dir.join("Подсистема.mdo");

        let mut obj =
            MetadataObject::new(ObjectKind::new("Subsystem"), "Подсистема", Uuid([1; 16]));
        attach_command_interface(Format::Edt, "Subsystem", &descriptor, &mut obj).unwrap();
        let ci = obj.command_interface.expect("command-interface attached");
        assert_eq!(ci.commands.len(), 2);
        assert!(!ci.commands[0].common_visible);
        assert!(ci.commands[1].common_visible);

        let _ = std::fs::remove_dir_all(&base);
    }
}
