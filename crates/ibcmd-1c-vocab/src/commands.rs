//! Command shortcuts: the `{0,<virtual key>,<modifier mask>}` tuple and the
//! spelling the platform writes for it; a command's picture descriptor; the
//! standard command a metadata kind is opened by.

use std::collections::BTreeMap;

use ibcmd_1c_text::{parse_1c_bool_flag, split_1c_braced_fields};

use crate::standard_picture_name;

/// Names the platform writes for the shortcut bitmask's CTRL and ALT bits.
///
/// The bits themselves are host-neutral -- the same `.cf` carries the same
/// `{0, virtual-key, modifier-mask}` tuple everywhere -- but `ibcmd` spells
/// them for the host it runs on, and that is the only thing that changes:
/// bit 8 is `Ctrl` on Windows and `Cmd` on macOS, bit 16 is `Alt` on Windows
/// and `Option` on macOS. Bit 4 stays `Shift`, the key name never moves, and
/// the modifier order is the same on both.
///
/// Measured on БСП базовая: of the 81 forms carrying `<Shortcut>`, 50 are
/// already byte-identical to the macOS reference tree and the other 31 become
/// byte-identical under exactly this substitution, with no residual
/// difference anywhere. The stand's reference trees are macOS captures
/// throughout -- 2 900 `Cmd` and 140 `Option` across uh/ut/ssl/sslbase, and
/// not one `Ctrl` or `Alt` -- so on this host the macOS spelling is what the
/// platform bytes say.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShortcutModifierStyle {
    Windows,
    Macos,
}

impl ShortcutModifierStyle {
    /// The spelling `ibcmd` uses on the host this export is running on.
    pub const fn host() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Windows
        }
    }

    pub const fn ctrl(self) -> &'static str {
        match self {
            Self::Windows => "Ctrl",
            Self::Macos => "Cmd",
        }
    }

    pub const fn alt(self) -> &'static str {
        match self {
            Self::Windows => "Alt",
            Self::Macos => "Option",
        }
    }
}

pub fn parse_common_command_shortcut_value(value: &str) -> Option<String> {
    parse_common_command_shortcut_value_with_style(value, ShortcutModifierStyle::host())
}

pub fn parse_common_command_shortcut_value_with_style(
    value: &str,
    style: ShortcutModifierStyle,
) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "0" {
        return None;
    }
    let key_code = fields.get(1)?.trim().parse::<u16>().ok()?;
    let modifier_code = fields.get(2)?.trim().parse::<u16>().ok()?;
    const SHIFT: u16 = 4;
    const CTRL: u16 = 8;
    const ALT: u16 = 16;
    if key_code == 0 || modifier_code & !(SHIFT | CTRL | ALT) != 0 {
        return None;
    }
    let key = match key_code {
        8 => "BackSpace".to_string(),
        13 => "Enter".to_string(),
        48..=57 => char::from_u32(u32::from(key_code))?.to_string(),
        65..=90 => char::from_u32(u32::from(key_code))?.to_string(),
        96..=105 => format!("Num {}", key_code - 96),
        // 106..=111 continue the VK_NUMPAD0..VK_DIVIDE numpad run 96..=105
        // already maps (VK_MULTIPLY=106, VK_ADD=107, VK_SEPARATOR=108,
        // VK_SUBTRACT=109, VK_DECIMAL=110 already spelled `Num .`,
        // VK_DIVIDE=111). Evidenced: `ssl`-demo's `DataProcessors/
        // КонсольЗапросов/Forms/Форма` carries key_code 111 twice (modifier 8
        // and 12), native `Cmd+Num /` and `Cmd+Shift+Num /`.
        // VK_ADD=107 and VK_SUBTRACT=109, the two of the 106..=109 run the
        // stand spells. ERP УХ 3.2.12.6 `Reports/ДеревоСебестоимостиПродукции/
        // Forms/ФормаОтчета` and
        // `Reports/СтруктураЗаказаНаПроизводствоДинамическая/Forms/ФормаОтчета`
        // each carry `{0,107,12}` and `{0,109,12}` on their expand-all and
        // collapse-all commands, and the platform writes `Cmd+Shift+Num +` and
        // `Cmd+Shift+Num -` for them. VK_MULTIPLY=106 and VK_SEPARATOR=108
        // occur nowhere in the eight trees and stay unread rather than guessed
        // from the run.
        //
        // VK_MULTIPLY=106 does occur: the eight native stand trees spell
        // `<Shortcut>Num *</Shortcut>` twice, on the `КнопкаУмножить` command
        // of ERP УХ 3.2.12.6
        // `DataProcessors/НастройкаСтруктурыОтчета/Forms/ФормаКонструктора`
        // and of `Catalogs/БланкиОтчетов/Forms/ФормаМакета`, so the census
        // that declared the code absent had not seen them. Seed `hg7-a`
        // against 8.3.27.2214 measures the code itself: a form command written
        // `<Shortcut>Num *</Shortcut>` stores `{0,106,0}` and its sibling
        // written `<Shortcut>Num /</Shortcut>` stores `{0,111,0}`, the code
        // this run already spells. VK_SEPARATOR=108 is spelled by no tree and
        // no seed and stays unread.
        106 => "Num *".to_string(),
        107 => "Num +".to_string(),
        109 => "Num -".to_string(),
        110 => "Num .".to_string(),
        111 => "Num /".to_string(),
        112..=123 => format!("F{}", key_code - 111),
        // VK_ESCAPE. The one unmapped key code any of the three probed
        // configurations spells: Документооборот КОРП 3.0.21.3 carries it once
        // with no modifier, on the very command the platform writes
        // `<Shortcut>Esc</Shortcut>` for -- the only `Esc` in the eight native
        // stand trees -- and УТ 11.5.27.75 spells no unmapped code at all.
        27 => "Esc".to_string(),
        _ => return None,
    };
    let mut parts = Vec::with_capacity(4);
    if modifier_code & CTRL != 0 {
        parts.push(style.ctrl().to_string());
    }
    if modifier_code & ALT != 0 {
        parts.push(style.alt().to_string());
    }
    if modifier_code & SHIFT != 0 {
        parts.push("Shift".to_string());
    }
    parts.push(key);
    Some(parts.join("+"))
}

// From `src/mssql_dump/mod.rs`.
pub fn parse_common_command_picture_value(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(Option<String>, bool)> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let picture_kind = fields.get(1)?.trim().parse::<i32>().ok()?;
    let load_transparent = fields
        .get(6)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    if picture_kind == 0 {
        return Some((None, load_transparent));
    }
    if picture_kind == 1 {
        let ref_fields = split_1c_braced_fields(fields.get(2)?, 0)?;
        if ref_fields.first()?.trim() == "-1" {
            return Some((
                Some("StdPicture.InputFieldSelect".to_string()),
                load_transparent,
            ));
        }
        if ref_fields.first()?.trim() == "-2" {
            return Some((
                Some("StdPicture.InputFieldClear".to_string()),
                load_transparent,
            ));
        }
        if ref_fields.first()?.trim() == "-3" {
            return Some((Some("StdPicture.MoveUp".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-4" {
            return Some((Some("StdPicture.MoveDown".to_string()), load_transparent));
        }
        // `-5` was the one negative code this table lacked while its sibling
        // table for help-topic picture tokens already carried it, so the two
        // disagreed about the same platform identity. The native tree writes
        // `StdPicture.InputFieldCalendar` for all 12 of its occurrences.
        if ref_fields.first()?.trim() == "-5" {
            return Some((
                Some("StdPicture.InputFieldCalendar".to_string()),
                load_transparent,
            ));
        }
        if ref_fields.first()?.trim() == "-9" {
            return Some((Some("StdPicture.MoveRight".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-8" {
            return Some((Some("StdPicture.MoveLeft".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-7" {
            return Some((
                Some("StdPicture.InputFieldOpen".to_string()),
                load_transparent,
            ));
        }
        if ref_fields.first()?.trim() == "-10" {
            return Some((Some("StdPicture.CheckAll".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-11" {
            return Some((Some("StdPicture.UncheckAll".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-13" {
            return Some((Some("StdPicture.Print".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-14" {
            return Some((
                Some("StdPicture.InputFieldChooseType".to_string()),
                load_transparent,
            ));
        }
        // `-15` and `-16` continue the same ordinal run as the codes above and
        // were the only two negative codes the reference tree observes that this
        // table did not carry: over all 5 200 forms `-15` reads
        // `StdPicture.ZoomOut` on all 8 of its occurrences and `-16`
        // `StdPicture.ZoomIn` on all 7, with no counter-example and no other
        // table in the crate mapping either code in a picture context.
        if ref_fields.first()?.trim() == "-15" {
            return Some((Some("StdPicture.ZoomOut".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-16" {
            return Some((Some("StdPicture.ZoomIn".to_string()), load_transparent));
        }
        // The corpus-wide source difference leaves one further published
        // negative picture code: `-100` is paired with `StdPicture.Select`.
        // It occurs once; no other negative descriptor or native picture name
        // remains unmatched after the already enumerated codes are removed.
        if ref_fields.first()?.trim() == "-100" {
            return Some((Some("StdPicture.Select".to_string()), load_transparent));
        }
        // The last two codes the reference trees observe and this table did not
        // carry.  Census over all 12 895 ERP УХ 3.2.12.6 form layouts and all
        // 1 738 Документооборот КОРП 3.0.21.3 ones, pairing each layout's
        // one-member negative reference descriptors against the
        // `StdPicture.*` names the native body of the same form writes and
        // subtracting the codes this table already spells: exactly two codes
        // are left unspelled anywhere, `-6` in 3 records and `-200` in 6, and
        // exactly two names are left unexplained, `StdPicture.
        // InputFieldCalculator` and `StdPicture.Clear`, each alone with its
        // code in every form that carries it and never seen beside the other.
        // `-6` also has a second, independent authority inside this crate:
        // `spreadsheet_standard_picture_ref` already writes it for
        // `StdPicture.InputFieldCalculator` on the packing side.
        //
        // `-200` is off the ordinal run the codes above form, which is why it
        // is spelled out rather than interpolated: nothing between `-17` and
        // `-199` occurs in any reference tree, so the run says nothing about
        // what those codes would mean.
        if ref_fields.first()?.trim() == "-6" {
            return Some((
                Some("StdPicture.InputFieldCalculator".to_string()),
                load_transparent,
            ));
        }
        if ref_fields.first()?.trim() == "-200" {
            return Some((Some("StdPicture.Clear".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "0" {
            // Same bare-index reference the register table admits; reading past
            // the end of a one-element reference used to abort the whole
            // descriptor here, which is a third answer neither table means.
            let Some(uuid) = ref_fields.get(1).map(|field| field.trim()) else {
                return Some((Some("0".to_string()), load_transparent));
            };
            if let Some(reference) = standard_picture_name(uuid) {
                return Some((Some(reference.to_string()), load_transparent));
            }
            if let Some(reference) = object_refs.get(uuid)
                && reference.starts_with("CommonPicture.")
            {
                return Some((Some(reference.clone()), load_transparent));
            }
            // A uuid that names nothing in the configuration is still a
            // reference the platform writes: it spells it `<code>:<uuid>` --
            // the descriptor's own two members, joined -- rather than dropping
            // the picture.  Answering `None` here dropped the whole `<Picture>`
            // element, not just the name.
            //
            // Census over ERP УХ 3.2.12.6, Документооборот КОРП 3.0.21.3,
            // УТ 11.5.27.75, БСП демо и БСП базовая: of the 63 449 `<xr:Ref>`
            // values those five trees write, exactly three spellings occur --
            // 39 566 `CommonPicture.<name>`, 23 873 `StdPicture.<name>` and 16
            // of this shape, all of them `0:<uuid>`, all in ERP УХ, over 10
            // distinct uuids.  Every one of those uuids occurs *only* inside
            // form bodies and names no object anywhere in the configuration,
            // which is why the lookups above cannot answer them; the one that
            // also occurs outside a form body is an equally dangling
            // `<xr:Item xsi:type="xr:MDObjectRef">` of a subsystem's content
            // list, written there as the bare uuid.
            //
            // Restricted to a uuid the object index does not carry at all: a
            // uuid that does resolve, to something that is not a common
            // picture, is a different question this corpus does not observe.
            if !object_refs.contains_key(uuid) {
                return Some((Some(format!("0:{uuid}")), load_transparent));
            }
        }
    }
    Some((None, load_transparent))
}

// From `src/mssql_dump/command_interface.rs`.
pub fn command_interface_standard_command(kind: &str) -> Option<&'static str> {
    match kind {
        "DataProcessor" | "Report" | "CommonForm" => Some("Open"),
        "AccountingRegister"
        | "AccumulationRegister"
        | "BusinessProcess"
        | "Catalog"
        | "ChartOfAccounts"
        | "ChartOfCalculationTypes"
        | "ChartOfCharacteristicTypes"
        | "Document"
        | "DocumentJournal"
        | "Enum"
        | "ExchangePlan"
        | "InformationRegister"
        | "Task" => Some("OpenList"),
        _ => None,
    }
}

// From `src/mssql_dump/command_interface.rs`.
pub fn command_interface_standard_command_for_code(kind: &str, code: &str) -> Option<&'static str> {
    match (code, kind) {
        ("0" | "100", "Constant") => Some("Open"),
        ("0" | "100", "CalculationRegister") => Some("OpenList"),
        ("0" | "100", _) => command_interface_standard_command(kind),
        // `ChartOfAccounts` names its "create" command from the same slot its
        // siblings do; it was simply never listed. ERP УХ 3.2.12.6 spells
        // exactly two, both `ChartOfAccounts.Хозрасчетный.StandardCommand.Create`
        // (`Subsystems/РегламентированныйУчет/Subsystems/БухгалтерскийИНалоговыйУчет`
        // and `Subsystems/РегламентированныйУчетПодсистемы/Subsystems/БухгалтерскийУчет`),
        // and both were written as the raw `1:<uuid>` sentinel instead.
        (
            "1",
            "BusinessProcess" | "Catalog" | "ChartOfAccounts" | "Document" | "InformationRegister",
        ) => Some("Create"),
        ("1", "ChartOfCharacteristicTypes") => Some("CreateFolder"),
        ("2", "Catalog") => Some("CreateFolder"),
        ("2", "ChartOfCharacteristicTypes") => Some("Create"),
        _ => None,
    }
}
