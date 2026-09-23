//! Writer for the managed-form item records a native 8.3.27 body stores.
//!
//! The base-free creator in `module_blob` writes the smallest record that
//! names an item and leaves the platform to fill the rest; a body it produces
//! is loadable but is not the body the platform would have stored. This module
//! is the other direction: each function returns the record the platform
//! itself writes, byte for byte, so a loaded configuration exports back to the
//! tree it came from.
//!
//! Every record here is measured against bodies read out of
//! `ibcmd_rs_uha_8327_parity2_20260920` with `mssql-dump-config --inflate`,
//! and the tests below hold the exact strings those bodies carry. A property
//! this module cannot yet write is not defaulted: the caller keeps refusing
//! the item, which is what the blocker model already does.

use std::collections::BTreeMap;
use std::sync::Arc;

/// The namespace every form *item* id lives in.
///
/// One uuid across all 3 971 item records of the four ERP УХ 3.3.3.3 form
/// bodies read for this module, from four unrelated families. Form attributes
/// live in a different one, which is why this is an item-only constant.
pub(crate) const FORM_ITEM_NAMESPACE_UUID: &str = "02023637-7868-4a5f-8576-835a76e0c9ba";

/// The palette reference every default appearance slot carries.
const DEFAULT_APPEARANCE_UUID: &str = "48312c09-257f-4b29-b280-284dd89efc1e";

/// The uuid a parent writes before a child to say what kind of child it is.
///
/// One uuid per record wrapper, with no overlap, over the child lists of the
/// first 400 ERP УХ form bodies: 2 353 fields, 2 198 groups, 1 309 buttons,
/// 677 decorations, 180 tables and 8 search additions.
pub(crate) const fn child_kind_uuid(wrapper: u8) -> Option<&'static str> {
    Some(match wrapper {
        37 => "77ffcc29-7f2d-4223-b22f-19666e7250ba",
        22 => "cd5394d0-7dda-4b56-8927-93ccbe967a01",
        31 => "a9f3b1ac-f51b-431e-b102-55a69acdecad",
        12 => "3d3cb80c-508b-41fa-8a18-680cdf5f1712",
        55 => "143c00f7-a42d-4cd7-9189-88e4467dc768",
        5 => "c5259a1d-518a-4afd-b98d-0176027e4feb",
        _ => return None,
    })
}

/// A 1C string literal: quoted, with `"` doubled.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// A colour a form item names, in the shape a body stores it.
///
/// Four shapes, each read off the corpus and then measured against it:
///
/// - a colour the item does not name is `{3,4,{0}}`;
/// - `#RRGGBB` is `{3,0,{n}}` where `n` is the same three bytes in the other
///   order, blue first -- 276 of 276 absolute colours of the corpus agree,
///   with none left over;
/// - `style:<one of the platform's own>` is `{3,3,{-n}}`, with the code below;
/// - `style:<one the configuration declares>` is `{3,3,{0,<its uuid>}}`, which
///   the caller resolves because only the configuration knows it;
/// - `web:<name>` is `{3,2,{n}}`.
///
/// A spelling this cannot place is refused rather than defaulted: writing the
/// wrong colour would load a body the export reads back differently.
pub(crate) fn format_native_color(
    value: Option<&str>,
    style_item_uuid: impl FnOnce(&str) -> Option<String>,
) -> Option<String> {
    let Some(value) = value else {
        return Some("{3,4,{0}}".to_string());
    };
    // `auto`, the platform's automatic colour: `{3,4,{-1}}` on every button
    // of both corpora that spells it.
    if value == "auto" {
        return Some("{3,4,{-1}}".to_string());
    }
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let red = u32::from_str_radix(&hex[0..2], 16).ok()?;
        let green = u32::from_str_radix(&hex[2..4], 16).ok()?;
        let blue = u32::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(format!("{{3,0,{{{}}}}}", (blue << 16) | (green << 8) | red));
    }
    if let Some(name) = value.strip_prefix("web:") {
        let code = WEB_COLOR_CODES
            .iter()
            .find_map(|(candidate, code)| (*candidate == name).then_some(*code))?;
        return Some(format!("{{3,2,{{{code}}}}}"));
    }
    if let Some(name) = value.strip_prefix("win:") {
        let code = WINDOWS_COLOR_CODES
            .iter()
            .find_map(|(candidate, code)| (*candidate == name).then_some(*code))?;
        return Some(format!("{{3,1,{{{code}}}}}"));
    }
    let name = value.strip_prefix("style:")?;
    if let Some(code) = PLATFORM_STYLE_COLOR_CODES
        .iter()
        .find_map(|(candidate, code)| (*candidate == name).then_some(*code))
    {
        return Some(format!("{{3,3,{{{code}}}}}"));
    }
    style_item_uuid(name).map(|uuid| format!("{{3,3,{{0,{uuid}}}}}"))
}

/// The codes the platform's own style colours carry, as the corpus spells
/// them.
const PLATFORM_STYLE_COLOR_CODES: &[(&str, &str)] = &[
    ("AccentColor", "-46"),
    ("ActivityColor", "-44"),
    ("AuxiliaryNavigationColor", "-43"),
    ("BorderColor", "-22"),
    ("ButtonBackColor", "-7"),
    ("ButtonBorderColor", "-34"),
    ("ButtonTextColor", "-21"),
    ("FieldAlternativeBackColor", "-13"),
    ("FieldBackColor", "-10"),
    ("FieldSelectedTextColor", "-15"),
    ("FieldSelectionBackColor", "-14"),
    ("FieldTextColor", "-11"),
    ("FormBackColor", "-1"),
    ("FormTextColor", "-3"),
    ("ImportantColor", "-47"),
    ("NavigationColor", "-42"),
    ("NegativeTextColor", "-17"),
    ("ReportGroup1BackColor", "-26"),
    ("ReportGroup2BackColor", "-27"),
    ("ReportHeaderBackColor", "-25"),
    ("ReportLineColor", "-28"),
    ("SpecialTextColor", "-16"),
    ("TableFooterBackColor", "-37"),
    ("TableHeaderBackColor", "-35"),
    ("TableHeaderTextColor", "-36"),
    ("ToolTipBackColor", "-23"),
    ("ToolTipTextColor", "-24"),
];

/// A `<Shortcut>` as a body stores it: `{0,<virtual-key code>,<modifiers>}`,
/// Shift 4, Ctrl 8 and Alt 16 summed. The inverse of the reader's table,
/// measured on 2 451 ERP УХ and 148 BSP command records and 112 item records:
/// letters and digits are their upper-case ASCII, `F1`..`F12` 112..123,
/// `Num 0`..`Num 9` 96..105, `Num *` 106, `Num +` 107, `Num -` 109, `Num .`
/// 110, `Num /` 111, `BackSpace` 8, `Enter` 13, `Esc` 27. Any other key is
/// refused.
pub(crate) fn format_native_shortcut(text: &str) -> Option<String> {
    let text = text.trim();
    // `Num +` ends in the separator itself, so it is split off first.
    let (modifiers, key) = if let Some(prefix) = text.strip_suffix("Num +") {
        (prefix.strip_suffix('+').unwrap_or(prefix), "Num +")
    } else {
        match text.rsplit_once('+') {
            Some((prefix, key)) => (prefix, key),
            None => ("", text),
        }
    };
    let code: u32 = match key {
        "BackSpace" => 8,
        "Enter" => 13,
        "Esc" => 27,
        "Num *" => 106,
        "Num +" => 107,
        "Num -" => 109,
        "Num ." => 110,
        "Num /" => 111,
        _ => {
            if let Some(digit) = key.strip_prefix("Num ") {
                let digit: u32 = digit.parse().ok()?;
                if digit > 9 {
                    return None;
                }
                96 + digit
            } else if let Some(number) = key.strip_prefix('F').filter(|rest| !rest.is_empty()) {
                let number: u32 = number.parse().ok()?;
                if !(1..=12).contains(&number) {
                    return None;
                }
                111 + number
            } else {
                let mut chars = key.chars();
                let single = chars.next()?;
                if chars.next().is_some() || !(single.is_ascii_uppercase() || single.is_ascii_digit()) {
                    return None;
                }
                u32::from(single)
            }
        }
    };
    let mut mask = 0u32;
    if !modifiers.is_empty() {
        for modifier in modifiers.split('+') {
            mask |= match modifier {
                "Shift" => 4,
                "Ctrl" => 8,
                "Alt" => 16,
                _ => return None,
            };
        }
    }
    Some(format!("{{0,{code},{mask}}}"))
}

/// The web colours, with the index a body stores: the ones measured on form
/// items, and the rest of the table the exporter reads the index back with.
const WEB_COLOR_CODES: &[(&str, &str)] = &[
    ("AliceBlue", "1"),
    ("Beige", "6"),
    ("Black", "8"),
    ("Blue", "10"),
    ("CadetBlue", "14"),
    ("Cream", "20"),
    ("Crimson", "21"),
    ("DarkBlue", "23"),
    ("DarkGray", "26"),
    ("DarkGreen", "27"),
    ("DarkOliveGreen", "30"),
    ("DarkOrange", "31"),
    ("DarkRed", "33"),
    ("DarkSlateGray", "37"),
    ("DeepSkyBlue", "41"),
    ("DimGray", "42"),
    ("DodgerBlue", "43"),
    ("FireBrick", "44"),
    ("FloralWhite", "45"),
    ("ForestGreen", "46"),
    ("Gainsboro", "48"),
    ("GhostWhite", "49"),
    ("Gold", "50"),
    ("Goldenrod", "51"),
    ("Gray", "52"),
    ("Green", "53"),
    ("HoneyDew", "55"),
    ("IndianRed", "57"),
    ("Lavender", "61"),
    ("LavenderBlush", "62"),
    ("LemonChiffon", "64"),
    ("LightBlue", "65"),
    ("LightCoral", "66"),
    ("LightCyan", "67"),
    ("LightGoldenRod", "68"),
    ("LightGoldenRodYellow", "69"),
    ("LightGray", "71"),
    ("LightGreen", "70"),
    ("LightPink", "72"),
    ("LightSalmon", "73"),
    ("LightSkyBlue", "75"),
    ("LightSlateGray", "77"),
    ("LightSteelBlue", "78"),
    ("LightYellow", "79"),
    ("Lime", "80"),
    ("Maroon", "84"),
    ("MediumBlue", "86"),
    ("MediumGray", "87"),
    ("MediumSeaGreen", "91"),
    ("MintCream", "97"),
    ("MistyRose", "98"),
    ("Moccasin", "96"),
    ("NavajoWhite", "100"),
    ("Orange", "105"),
    ("OrangeRed", "106"),
    ("PaleGreen", "109"),
    ("PaleTurquoise", "110"),
    ("PapayaWhip", "112"),
    ("Pink", "115"),
    ("PowderBlue", "117"),
    ("Red", "119"),
    ("RosyBrown", "120"),
    ("RoyalBlue", "121"),
    ("SaddleBrown", "122"),
    ("Salmon", "123"),
    ("Sienna", "127"),
    ("Silver", "128"),
    ("SkyBlue", "129"),
    ("SlateBlue", "130"),
    ("SlateGray", "131"),
    ("Snow", "132"),
    ("SteelBlue", "134"),
    ("Violet", "140"),
    ("VioletRed", "141"),
    ("White", "143"),
    ("WhiteSmoke", "144"),
    ("Yellow", "145"),
];

/// The Windows system colours the corpus names, with the index a body stores
/// under `{3,1,{n}}` -- the ones the form exporter reads back.
const WINDOWS_COLOR_CODES: &[(&str, &str)] = &[
    ("ActiveTitleBar", "2"),
    ("ButtonDarkShadow", "21"),
    ("ButtonText", "18"),
    ("DisabledText", "17"),
    ("MenuBar", "4"),
    ("ScrollBar", "0"),
];

/// One `<Event>` of an item, as the source names it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeEvent<'a> {
    /// The event's name, such as `OnChange`; a form the platform could not
    /// spell writes the event's uuid here instead, and it stands for itself.
    pub(crate) name: &'a str,
    /// The procedure the event calls.
    pub(crate) handler: &'a str,
}

/// The event bindings of one owner, in the shape a body stores them.
///
/// Measured over the 12 507 forms of ERP УХ: 63 161 of 63 161 stored tuples
/// rebuild byte for byte from the source alone. The shape is
///
/// ```text
/// { N, (<uuid>,"<handler>") x N, 1, 0, (<uuid>,0,<M>,("<handler>",1) x (M-1)) x N }
/// ```
///
/// where `N` counts the distinct events and `M` counts the handlers one event
/// carries: an extension adds its own handler beside the base one, and the two
/// share the event's name, so the group -- not the `<Event>` -- is the unit.
/// An owner with no events writes `{0,1,0}`.
///
/// The uuid is the event's identity, and it depends on who declares the event:
/// `OnChange` of a field is not `OnChange` of a table. For a form-level event
/// it depends further on the class of the form's main attribute, because that
/// is what picks the form extension declaring it -- `BeforeWrite` of a document
/// form and of every other form are two different events. A name the table does
/// not hold is refused rather than guessed.
pub(crate) fn format_native_events(
    owner_tag: &str,
    main_attribute_class: &str,
    events: &[NativeEvent<'_>],
) -> Option<String> {
    if events.is_empty() {
        return Some("{0,1,0}".to_string());
    }
    // Events group by name, in the order the source first names each one.
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for event in events {
        match groups.iter_mut().find(|(name, _)| *name == event.name) {
            Some((_, handlers)) => handlers.push(event.handler),
            None => groups.push((event.name, vec![event.handler])),
        }
    }
    let mut uuids = Vec::with_capacity(groups.len());
    for (name, _) in &groups {
        uuids.push(form_event_uuid(owner_tag, main_attribute_class, name)?);
    }
    let head = groups
        .iter()
        .zip(&uuids)
        .map(|((_, handlers), uuid)| format!("{uuid},{}", quoted(handlers[0])))
        .collect::<Vec<_>>()
        .join(",");
    let tail = groups
        .iter()
        .zip(&uuids)
        .map(|((_, handlers), uuid)| {
            let extra = handlers[1..]
                .iter()
                .map(|handler| format!(",{},1", quoted(handler)))
                .collect::<String>();
            format!("{uuid},0,{}{extra}", handlers.len())
        })
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("{{{},{head},1,0,{tail}}}", groups.len()))
}

/// The uuid of one event, by the kind that declares it.
fn form_event_uuid<'a>(owner_tag: &str, main_attribute_class: &str, name: &'a str) -> Option<&'a str> {
    if is_uuid(name) {
        return Some(name);
    }
    // The main attribute narrows a form-level event first; every other event,
    // and every form whose main attribute the table does not single out, reads
    // the entry that leaves the class open.
    FORM_EVENT_UUIDS
        .iter()
        .find(|(tag, class, candidate, _)| {
            *tag == owner_tag && *class == main_attribute_class && *candidate == name
        })
        .or_else(|| {
            FORM_EVENT_UUIDS.iter().find(|(tag, class, candidate, _)| {
                *tag == owner_tag && class.is_empty() && *candidate == name
            })
        })
        .map(|(_, _, _, uuid)| *uuid)
}

/// Whether a name is already an event uuid, which the platform writes when it
/// has no spelling for the event.
fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.as_bytes().iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// `(owner tag, main attribute class, event name, uuid)`, read off the corpus.
/// An empty class matches any form; only `BeforeWrite` and `BeforeWriteAtServer`
/// of a form need the class, and only a document form parts company there.
const FORM_EVENT_UUIDS: &[(&str, &str, &str, &str)] = &[
    ("CalendarField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("CalendarField", "", "OnPeriodOutput", "1490ede6-6f33-4c6d-b971-53b2541331ea"),
    ("CalendarField", "", "Selection", "2feb1ee9-b750-4352-bb4c-67ba1c608dc6"),
    ("ChartField", "", "DetailProcessing", "650da4af-3233-4ce0-a1ae-23f87a226eee"),
    ("ChartField", "", "Selection", "515cd17b-dd4c-4181-bbbf-8676467acf49"),
    ("CheckBoxField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("ExtendedTooltip", "", "Click", "11707a99-4eb9-4373-bc8c-84891483a034"),
    ("ExtendedTooltip", "", "URLProcessing", "d710ea07-5c96-4c43-ab6e-e138d3653780"),
    ("Form", "", "047d4d09-961c-4bdc-8519-eef10674c35b", "047d4d09-961c-4bdc-8519-eef10674c35b"),
    ("Form", "", "213d1900-dcad-4616-9f20-3f077156a40f", "213d1900-dcad-4616-9f20-3f077156a40f"),
    ("Form", "", "390d5e4b-e732-4c88-8748-9e211a416984", "390d5e4b-e732-4c88-8748-9e211a416984"),
    ("Form", "", "8f42e083-be92-4102-b1f0-fa58452c1a63", "8f42e083-be92-4102-b1f0-fa58452c1a63"),
    ("Form", "", "9cc34712-da5f-4faa-a653-343d2085fbe8", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "", "ActivationProcessing", "b47699e1-b5d8-4c8a-91e9-183dec5820f5"),
    ("Form", "", "AfterWrite", "047d4d09-961c-4bdc-8519-eef10674c35b"),
    ("Form", "", "AfterWriteAtServer", "213d1900-dcad-4616-9f20-3f077156a40f"),
    ("Form", "", "BeforeClose", "52dbb775-1631-4fd5-8c55-1615b5881dac"),
    ("Form", "", "BeforeLoadDataFromSettingsAtServer", "e773807c-0c0c-4689-a093-231ddcd6409f"),
    ("Form", "", "BeforeLoadUserSettingsAtServer", "40925042-2517-455b-a600-d68282829334"),
    ("Form", "", "BeforeLoadVariantAtServer", "1dd89674-8b50-4240-9899-e3426b79cb02"),
    ("Form", "", "ChoiceProcessing", "1d632984-de3c-4b4b-ad9f-d69682a10182"),
    ("Form", "", "ExternalEvent", "5426e344-5740-4f23-99c1-99179a200dc5"),
    ("Form", "", "FillCheckProcessingAtServer", "e73d6384-49d2-4885-a752-a674d6ff7742"),
    ("Form", "", "NavigationProcessing", "93dfba16-26db-46f8-acb5-4f92f50c855f"),
    ("Form", "", "NewWriteProcessing", "3b644f4f-055f-4808-bdc6-a50ce895e4d9"),
    ("Form", "", "NotificationProcessing", "3699f6a3-9a2a-4c82-a775-6ff4824a08ca"),
    ("Form", "", "OnChangeDisplaySettings", "b98da5a8-349c-4159-a6a8-17a34ceb10ec"),
    ("Form", "", "OnClose", "ca21cd18-35b2-4281-b5c8-016ecc8da8ac"),
    ("Form", "", "OnCreateAtServer", "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b"),
    ("Form", "", "OnLoadDataFromSettingsAtServer", "79cea13e-f6fb-4483-905d-713326405771"),
    ("Form", "", "OnLoadUserSettingsAtServer", "7b15b3db-1cd0-4e1d-a74b-2c972c9e2226"),
    ("Form", "", "OnLoadVariantAtServer", "87ce636e-9de6-4e42-9395-f0f189d08397"),
    ("Form", "", "OnMainServerAvailabilityChange", "d6b86f20-722b-4fe6-83fa-85c6aa4c1fe5"),
    ("Form", "", "OnOpen", "3ccc650e-f631-4cae-8e33-3eaac610b5f9"),
    ("Form", "", "OnReadAtServer", "390d5e4b-e732-4c88-8748-9e211a416984"),
    ("Form", "", "OnReopen", "6b3175a5-c143-4179-a670-ef231dc0a688"),
    ("Form", "", "OnSaveDataInSettingsAtServer", "1952a54f-35ad-4928-902f-df212ab38ca3"),
    ("Form", "", "OnSaveUserSettingsAtServer", "961ee7c6-0327-422b-adcb-97a90c46753d"),
    ("Form", "", "OnSaveVariantAtServer", "499bb7af-6262-4de4-819f-ef264d1a20ec"),
    ("Form", "", "OnUpdateUserSettingSetAtServer", "d817bccf-504e-4133-a79a-dd16e3a4df73"),
    ("Form", "", "OnWriteAtServer", "c1bc0d3e-d35e-4207-a06b-ece68ed25314"),
    ("Form", "", "URLGetProcessing", "674956b3-e469-4fdc-acf5-24ebf88cf7ab"),
    ("Form", "", "URLListGetProcessing", "44498116-1641-4bfa-ae33-86e53c205797"),
    ("Form", "", "URLProcessing", "e0cd9bdf-88fa-428c-9f1f-86f7f73b11e2"),
    ("Form", "", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "", "c1bc0d3e-d35e-4207-a06b-ece68ed25314", "c1bc0d3e-d35e-4207-a06b-ece68ed25314"),
    ("FormattedDocumentField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("GanttChartField", "", "DetailProcessing", "8724b8d4-140d-4357-8ac9-46e29ba7b168"),
    ("GanttChartField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    // The exporter's own table (form_schema.rs, `FORM_GANTT_CHART_*`): ERP УХ
    // `DataProcessors/ДиаграммаГантаОперации/Forms/Форма` stores the second.
    ("GanttChartField", "", "Selection", "3aab5acd-9e00-4d33-8242-3cdb677bb0f3"),
    ("GanttChartField", "", "OnIntervalEditEnd", "fe4544e7-5b1a-441c-8ab9-198137e6d3c7"),
    ("GraphicalSchemaField", "", "OnActivate", "83c14f85-ab1f-4c77-bd3b-81970b72543b"),
    ("GraphicalSchemaField", "", "Selection", "3c3da18f-fc18-4f77-8c2d-96c25bec40a5"),
    ("HTMLDocumentField", "", "DocumentComplete", "53325f0c-b112-4c44-ab12-5d1ee0b1f07b"),
    ("HTMLDocumentField", "", "OnClick", "da8dfb86-c5d1-4e35-a8a4-01b167a60ad3"),
    ("InputField", "", "AutoComplete", "178a97c4-0ffe-4fcc-93e6-505369939da5"),
    ("InputField", "", "ChoiceProcessing", "f72043b8-2d79-414e-bc4e-3972fe9dbca1"),
    ("InputField", "", "Clearing", "b50dc41b-c15a-4ebe-a17f-d01e51c47de6"),
    ("InputField", "", "Creating", "aeba313d-c467-44b3-b4a2-956340932c8f"),
    ("InputField", "", "EditTextChange", "14256303-d2b7-4a58-bfab-e77493d10a59"),
    ("InputField", "", "MultipleValuesDelete", "49ede602-af78-4a50-b821-ec81f6778f2d"),
    ("InputField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("InputField", "", "Opening", "ac5a9c5a-5f1d-4fc5-b88c-a187038c16d1"),
    ("InputField", "", "StartChoice", "1960479b-4d89-4eba-8b39-0aa802020558"),
    ("InputField", "", "StartListChoice", "b3b65989-73ac-4db3-b6cb-398cb41a062f"),
    ("InputField", "", "TextEditEnd", "c331eb1b-d32b-4533-844c-1276600b64e3"),
    ("InputField", "", "Tuning", "70636369-514c-4662-977e-1c3976c9756c"),
    ("LabelDecoration", "", "Click", "11707a99-4eb9-4373-bc8c-84891483a034"),
    ("LabelDecoration", "", "URLProcessing", "d710ea07-5c96-4c43-ab6e-e138d3653780"),
    ("LabelField", "", "Click", "eba5f295-c611-4dd9-84b5-22911ad60c53"),
    ("LabelField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("LabelField", "", "URLProcessing", "509eca20-d6e4-4fef-a0f8-3a6b44c64178"),
    ("Pages", "", "OnCurrentPageChange", "526c501f-ed3f-4db4-8731-fd0324707501"),
    ("PictureDecoration", "", "Click", "9874537f-454c-40ae-83e9-3b9cefbc6d08"),
    ("PictureDecoration", "", "Drag", "8ad48496-8d0b-4f6c-ae48-99d95227884b"),
    ("PictureDecoration", "", "DragCheck", "0d644ff6-443b-4390-86fa-7f9105e42711"),
    ("PictureField", "", "Click", "996b8c30-7a89-4973-8d56-2c9ce2976695"),
    ("PictureField", "", "Drag", "8ad48496-8d0b-4f6c-ae48-99d95227884b"),
    ("PictureField", "", "DragCheck", "0d644ff6-443b-4390-86fa-7f9105e42711"),
    ("PictureField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("RadioButtonField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("SpreadSheetDocumentField", "", "AdditionalDetailProcessing", "0b8dc702-d001-4637-a215-9f35613e096c"),
    ("SpreadSheetDocumentField", "", "BeforeWrite", "b7646583-04d3-4905-8f04-8985914bd1b7"),
    ("Table", "", "OnCurrentParentChange", "2971b9a9-1724-4f34-aaa4-f3db584c3ca0"),
    ("SpreadSheetDocumentField", "", "BeforePrint", "61455593-0982-4415-bc2e-2e8722a7abd0"),
    ("SpreadSheetDocumentField", "", "DetailProcessing", "2988b2a5-c887-4928-94ae-5d0c9c31e999"),
    ("SpreadSheetDocumentField", "", "Drag", "8ad48496-8d0b-4f6c-ae48-99d95227884b"),
    ("SpreadSheetDocumentField", "", "DragCheck", "0d644ff6-443b-4390-86fa-7f9105e42711"),
    ("SpreadSheetDocumentField", "", "DragEnd", "cb286ab3-3a1c-40d2-a232-6e64f624ccec"),
    ("SpreadSheetDocumentField", "", "DragStart", "6d4d6747-a823-4f61-ab31-a426572f2c6c"),
    ("SpreadSheetDocumentField", "", "OnActivate", "2042ec93-3108-4190-b767-ec6c10dd9ff4"),
    ("SpreadSheetDocumentField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("SpreadSheetDocumentField", "", "OnChangeAreaContent", "411a4578-276c-4f4a-b56a-b3b01181c997"),
    ("SpreadSheetDocumentField", "", "Selection", "22287505-97d8-4258-a318-209e2493f7eb"),
    ("SpreadSheetDocumentField", "", "URLProcessing", "06d41ccc-4e8a-46f8-aeff-b3303cf753d2"),
    ("Table", "", "2391e7b8-7235-45d7-ab7e-6ff3dc086396", "2391e7b8-7235-45d7-ab7e-6ff3dc086396"),
    ("Table", "", "2ccfdec5-583d-4eca-8319-e55de492665a", "2ccfdec5-583d-4eca-8319-e55de492665a"),
    ("Table", "", "4d88756d-bad4-4fde-92e1-c1f1402ac6b2", "4d88756d-bad4-4fde-92e1-c1f1402ac6b2"),
    ("Table", "", "AfterDeleteRow", "de65638d-a806-4a76-bc10-f62bbc86e0e7"),
    ("Table", "", "BeforeAddRow", "2391e7b8-7235-45d7-ab7e-6ff3dc086396"),
    ("Table", "", "BeforeCollapse", "a7a9dc42-29b6-4c5b-8980-6d0b87149bdd"),
    ("Table", "", "BeforeDeleteRow", "2ccfdec5-583d-4eca-8319-e55de492665a"),
    ("Table", "", "BeforeEditEnd", "4d88756d-bad4-4fde-92e1-c1f1402ac6b2"),
    ("Table", "", "BeforeExpand", "7c39b7bc-db0f-4410-9d98-8e5b7896995e"),
    ("Table", "", "BeforeLoadUserSettingsAtServer", "c41e7b98-098c-433e-8ac3-56ec2a2c49e2"),
    ("Table", "", "BeforeRowChange", "ab930362-ff94-4dcb-ad16-188805d23e3c"),
    ("Table", "", "ChoiceProcessing", "8bfdb5eb-62dc-4851-8a2c-e983526356bf"),
    ("Table", "", "Drag", "8ad48496-8d0b-4f6c-ae48-99d95227884b"),
    ("Table", "", "DragCheck", "0d644ff6-443b-4390-86fa-7f9105e42711"),
    ("Table", "", "DragEnd", "cb286ab3-3a1c-40d2-a232-6e64f624ccec"),
    ("Table", "", "DragStart", "6d4d6747-a823-4f61-ab31-a426572f2c6c"),
    ("Table", "", "NewWriteProcessing", "ce67decf-16b8-4d61-b347-4e6a063580dc"),
    ("Table", "", "OnActivateCell", "f228b12f-d892-4925-b338-695617357b32"),
    ("Table", "", "OnActivateField", "6e973761-8683-47fa-a609-4e230950294d"),
    ("Table", "", "OnActivateRow", "60edb81d-887b-478e-94ee-7fef2b13393d"),
    ("Table", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("Table", "", "OnEditEnd", "01d80ddd-dce5-4db3-beb5-f63c97cb05b9"),
    ("Table", "", "OnGetDataAtServer", "97365900-eadf-4dfd-a9aa-fbb9ecabd079"),
    ("Table", "", "OnLoadUserSettingsAtServer", "336b3ee5-d67f-4651-b098-e2c53f8317e2"),
    ("Table", "", "OnSaveUserSettingsAtServer", "a73dae96-734d-42e4-8ae7-b70249ecd233"),
    ("Table", "", "OnStartEdit", "b3c10170-c5ff-4cba-b537-679e1c872b45"),
    ("Table", "", "OnUpdateUserSettingSetAtServer", "e91128e6-621d-4dc8-b12e-bd65aeb37e2d"),
    ("Table", "", "RefreshRequestProcessing", "ff33c4d6-a0db-4906-992e-37b3f44cd97a"),
    ("Table", "", "Selection", "1282f000-23b6-4887-87f4-9e8e79db3d32"),
    ("Table", "", "URLGetProcessing", "674956b3-e469-4fdc-acf5-24ebf88cf7ab"),
    ("Table", "", "ValueChoice", "0d8cf5b0-55eb-4d1e-960a-22c160210945"),
    ("Table", "", "ab930362-ff94-4dcb-ad16-188805d23e3c", "ab930362-ff94-4dcb-ad16-188805d23e3c"),
    ("Table", "", "b3c10170-c5ff-4cba-b537-679e1c872b45", "b3c10170-c5ff-4cba-b537-679e1c872b45"),
    ("Table", "", "de65638d-a806-4a76-bc10-f62bbc86e0e7", "de65638d-a806-4a76-bc10-f62bbc86e0e7"),
    ("TextDocumentField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("TrackBarField", "", "OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    ("Form", "cfg:AccountingRegisterRecordSet", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:AccountingRegisterRecordSet", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:BusinessProcessObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:BusinessProcessObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:CatalogObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:CatalogObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:ChartOfAccountsObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:ChartOfCalculationTypesObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:ChartOfCalculationTypesObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:ChartOfCharacteristicTypesObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:ChartOfCharacteristicTypesObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:ConstantsSet", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:ConstantsSet", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:DocumentObject", "BeforeWrite", "8a5894c9-d2ff-4c1d-b433-89cc352bbfbc"),
    ("Form", "cfg:DocumentObject", "BeforeWriteAtServer", "8f42e083-be92-4102-b1f0-fa58452c1a63"),
    ("Form", "cfg:ExchangePlanObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:ExchangePlanObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:InformationRegisterRecordManager", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:InformationRegisterRecordManager", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:InformationRegisterRecordSet", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "cfg:InformationRegisterRecordSet", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "cfg:TaskObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
];

/// The font a form item names, in the shape a body stores it.
///
/// Measured over the label fields of 1 501 ERP УХ forms, where a `<Font>` is
/// either absent or one of three kinds:
///
/// - absent is `{7,3,0,1,100}` -- 4 968 records, with nothing else;
/// - `<Font kind="StyleItem" ref="style:<name>"/>` and nothing more is
///   `{7,2,0,{0,<its uuid>},1,100}`;
/// - a font that also sets `bold`, `italic` or a size grows a mask and a
///   weight -- `{7,2,60,{-31},700,0,0,0,1,100}` for a bold
///   `style:NormalTextFont` -- and a `WindowsFont` writes kind 1 with `{0}`.
///
/// Only the first two are written. The third is refused rather than guessed:
/// 33 records of that corpus carry it, which is not enough to say what the
/// mask counts.
pub(crate) fn format_native_font(
    attributes: &BTreeMap<String, String>,
    style_item_uuid: impl FnOnce(&str) -> Option<String>,
) -> Option<String> {
    // `{7,<kind>,<mask>[,<ref slot>],<values…>,1,<scale>}`.
    //
    // The mask carries one bit per optional attribute and the values follow in
    // an order that is not the one the XML spells them in. Built from the
    // source attributes alone this reproduces 8 660 of the 8 665 font elements
    // of both corpora byte for byte; the five it does not are `Absolute`,
    // which is a different shape and refuses below. An element that is absent
    // altogether is `{7,3,0,1,100}` in 1 269 821 of 1 269 821 item slots.
    if attributes.is_empty() {
        return Some("{7,3,0,1,100}".to_string());
    }
    let kind = match attributes.get("kind").map(String::as_str) {
        Some("WindowsFont") => "1",
        Some("StyleItem") => "2",
        Some("AutoFont") => "3",
        Some("Absolute") => return format_native_absolute_font(attributes),
        _ => return None,
    };
    let reference = attributes.get("ref").map(String::as_str);
    let slot = match (kind, reference) {
        ("3", None) => None,
        ("1", Some("sys:DefaultGUIFont")) => Some("{0}".to_string()),
        ("1", Some("sys:ANSIFixedFont")) => Some("{2}".to_string()),
        ("2", Some(reference)) => {
            let name = reference.strip_prefix("style:")?;
            match PLATFORM_STYLE_FONT_CODES
                .iter()
                .find(|(candidate, _)| *candidate == name)
            {
                Some((_, code)) => Some(format!("{{{code}}}")),
                None => Some(format!("{{0,{}}}", style_item_uuid(name)?)),
            }
        }
        _ => return None,
    };

    let mut mask = 0u32;
    let mut values = Vec::new();
    // `height` is tenths of a point, `bold` is 700 or 400, the three flags are
    // 1 or 0, and `faceName` is quoted. `scale` sets its bit but rides in the
    // tail rather than the value list.
    if let Some(height) = attributes.get("height") {
        mask |= 1 << 1;
        values.push((1u32, (height.trim().parse::<i64>().ok()? * 10).to_string()));
    }
    if let Some(bold) = attributes.get("bold") {
        mask |= 1 << 2;
        values.push((2, if native_font_flag(bold)? { "700" } else { "400" }.to_string()));
    }
    for (name, bit) in [("italic", 3u32), ("underline", 4), ("strikeout", 5)] {
        if let Some(value) = attributes.get(name) {
            mask |= 1 << bit;
            values.push((bit, u8::from(native_font_flag(value)?).to_string()));
        }
    }
    if let Some(face) = attributes.get("faceName") {
        mask |= 1 << 0;
        values.push((6, quoted(face)));
    }
    let scale = match attributes.get("scale") {
        Some(scale) => {
            mask |= 1 << 9;
            scale.trim().parse::<i64>().ok()?.to_string()
        }
        None => "100".to_string(),
    };
    // Every other key would be a spelling this rule has not measured.
    if attributes.keys().any(|key| {
        !matches!(
            key.as_str(),
            "kind"
                | "ref"
                | "faceName"
                | "height"
                | "bold"
                | "italic"
                | "underline"
                | "strikeout"
                | "scale"
        )
    }) {
        return None;
    }
    values.sort_by_key(|(order, _)| *order);

    let mut out = format!("{{7,{kind},{mask}");
    if let Some(slot) = slot {
        out.push(',');
        out.push_str(&slot);
    }
    for (_, value) in values {
        out.push(',');
        out.push_str(&value);
    }
    out.push_str(&format!(",1,{scale}}}"));
    Some(out)
}

/// A `kind="Absolute"` font, which is a fixed nineteen-member LOGFONT rather
/// than the mask-and-values tuple the other three kinds take.
///
/// Slots 11 to 15 are the LOGFONT's `charSet, outPrecision, clipPrecision,
/// quality, pitchAndFamily`, and the `<Font>` element carries none of them.
/// They hold `0,0,0,0,0` in 1 316 of the 1 321 elements of both corpora and
/// `1,3,2,1,34` in five, whose source spelling is identical to the majority's
/// -- so the source cannot choose. It does not have to: the platform's own
/// export of both shapes is the same `<Font/>` element, byte for byte, and
/// this crate's reader consults neither the mask nor slots 11 to 15 for kind
/// 0. The first export of a loaded configuration differs from the database in
/// those five records and the second is identical, which is the standard the
/// settings composer and the navigator are already written under.
///
/// Every one of the 1 321 spells all seven attributes, so a font that omits
/// one is unmeasured and refuses -- and the reader would make it worse than a
/// byte difference, because kind 0 emits all seven unconditionally and the
/// round trip would add the missing one to the source.
fn format_native_absolute_font(attributes: &BTreeMap<String, String>) -> Option<String> {
    let value = |name: &str| attributes.get(name).map(String::as_str);
    let flag = |name: &str| native_font_flag(value(name)?).map(u8::from);
    let height = value("height")?.trim().parse::<i64>().ok()? * 10;
    let weight = if native_font_flag(value("bold")?)? {
        "700"
    } else {
        "400"
    };
    let italic = flag("italic")?;
    let underline = flag("underline")?;
    let strikeout = flag("strikeout")?;
    let face_name = value("faceName")?;
    let scale = value("scale")?.trim().parse::<i64>().ok()?;
    if attributes.len() != 8 {
        // `kind` plus the seven. Anything else is a spelling the corpus never
        // showed, and the writer does not invent a LOGFONT for it.
        return None;
    }
    Some(format!(
        "{{7,0,575,{height},0,0,0,{weight},{italic},{underline},{strikeout},0,0,0,0,0,{},1,{scale}}}",
        quoted(face_name)
    ))
}

/// `true`/`false` as a font attribute spells it.
fn native_font_flag(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// The five platform fonts a `style:` reference can name, which no
/// `StyleItems\<name>.xml` declares. Their codes are a table of their own --
/// none of them collides with [`PLATFORM_STYLE_COLOR_CODES`].
const PLATFORM_STYLE_FONT_CODES: &[(&str, &str)] = &[
    ("ExtraLargeTextFont", "-33"),
    ("LargeTextFont", "-32"),
    ("NormalTextFont", "-31"),
    ("SmallTextFont", "-30"),
    ("TextFont", "-20"),
];

/// The `{12,…}` record of an item's `<ExtendedTooltip>`, as the platform
/// stores it when the tooltip carries nothing but its own name.
///
/// A single constant shape: all 85 tooltip records of
/// `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов` and
/// `BusinessProcesses/Задание/Forms/ДействиеВыполнить` normalise to it, with
/// only the id and the name differing.
pub(crate) fn format_extended_tooltip(id: &str, name: &str) -> String {
    format!(
        "{{12,{{{id},{ns}}},0,0,0,0,{name},{{1,0}},{{1,0}},1,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{5,0,0,3,0,{{0,1,0}},{{3,4,{{0}}}},{{3,4,{{0}}}},\
         {{3,0,{{0}},0,1,0,{appearance}}}}},0,1,2,{{1,{{1,0}},0}},0,0,1,0,0,1,0,3,3,0,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
        appearance = DEFAULT_APPEARANCE_UUID,
    )
}

/// The `{22,…}` record of a field's `<ContextMenu>`, as the platform stores it
/// when the menu carries nothing but its own name.
pub(crate) fn format_field_context_menu(
    id: &str,
    name: &str,
    autofill: Option<bool>,
    children: &[(&str, String)],
) -> String {
    // Two of the twenty-nine were constants the corpus contradicts, both
    // pure over 236 253 menus: member 21 is the payload, `{1,1}` when
    // `<Autofill>` is absent and `{1,0}` when it is `false`; member 22 is
    // the child count, 0 for the 233 070 menus with no `<ChildItems>` and n
    // for the 3 183 that have them, with the child records following.
    // `<Autofill>` with the lower-case f, for the third time in this writer.
    let mut records = String::new();
    for (kind_uuid, record) in children {
        records.push(',');
        records.push_str(kind_uuid);
        records.push(',');
        records.push_str(record);
    }
    format!(
        "{{22,{{{id},{ns}}},0,0,0,8,{name},{{1,0}},{{1,0}},0,1,0,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{1,{autofill}}},{count}{records},1,0,0,0,3,3,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
        autofill = u8::from(autofill.unwrap_or(true)),
        count = children.len(),
    )
}

/// A field, as the body stores it.
///
/// Member 4 is the same flag a container carries: when it is 1 a
/// functional-options block follows before the kind. With that block lifted
/// the record is **fixed-length, 59 members**, in 69 521 of the 69 526 input
/// fields of ERP УХ.
///
/// Measured over those 69 521: **69 456 rebuild byte for byte (99.91%)**, and
/// over every field kind of the corpus -- labels, check boxes, radio buttons,
/// pictures, documents and the rest, 122 767 records -- **122 630 (99.89%)**,
/// once
/// the members that name a configuration object come from the caller -- the
/// id, the name, the titles, the data paths, the pictures, the colours, the
/// font, the payload, the events, the context menu and the extended tooltip.
pub(crate) struct NativeFieldItem<'a> {
    pub(crate) id: &'a str,
    /// Which field this is -- see [`native_field_kind`].
    pub(crate) kind: u8,
    /// The functional-options block, when the field restricts itself.
    pub(crate) functional_options: Option<&'a str>,
    pub(crate) name: &'a str,
    /// `<TitleLocation>`: `None`, `Left`, `Top`, `Right` or `Bottom`.
    pub(crate) title_location: Option<&'a str>,
    /// `<TitleHeight>`.
    pub(crate) title_height: Option<&'a str>,
    /// Already formatted -- the title and the tooltip title.
    pub(crate) title: &'a str,
    pub(crate) tooltip_title: &'a str,
    /// The binding the item's `<DataPath>` resolves to, already formatted --
    /// `{1,{2}}` for a form attribute, `{2,{1},{3}}` for a dynamic-list column.
    pub(crate) data_path: &'a str,
    /// `<FooterDataPath>`, `{0}` when the field has none.
    pub(crate) footer_data_path: &'a str,
    pub(crate) enabled: bool,
    pub(crate) read_only: bool,
    /// `<SkipOnInput>`: 2 when the field names neither value.
    pub(crate) skip_on_input: Option<bool>,
    pub(crate) default_item: bool,
    /// `<WarningOnEditRepresentation>`: `Show` or `DontShow`.
    pub(crate) warning_on_edit: Option<&'a str>,
    /// The header's and the footer's titles, already formatted.
    pub(crate) header_title: &'a str,
    pub(crate) footer_title: &'a str,
    pub(crate) show_in_header: bool,
    pub(crate) show_in_footer: bool,
    pub(crate) cell_hyperlink: bool,
    /// `<HorizontalAlign>`, `<HeaderHorizontalAlign>`, `<FooterHorizontalAlign>`
    /// and `<VerticalAlign>`.
    pub(crate) horizontal_align: Option<&'a str>,
    pub(crate) header_horizontal_align: Option<&'a str>,
    pub(crate) footer_horizontal_align: Option<&'a str>,
    pub(crate) vertical_align: Option<&'a str>,
    /// `<EditMode>`: `Directly`, `EnterOnInput` or `Enter`.
    pub(crate) edit_mode: Option<&'a str>,
    pub(crate) auto_cell_height: bool,
    /// The header's and footer's pictures, already formatted.
    pub(crate) header_picture: &'a str,
    pub(crate) footer_picture: &'a str,
    /// The five appearance blocks and two fonts the field carries, already
    /// formatted -- see [`format_native_color`] and [`format_native_font`] --
    /// and last the block that closes the field's formats, `{0}` when it
    /// names none.
    pub(crate) appearance: [&'a str; 7],
    /// The picture index tuple, `{0,0,0}` when the field names none.
    pub(crate) picture_index: &'a str,
    /// The tuple that carries this kind's own properties: `{11,…}` for a
    /// label, `{36,…}` for an input, `{13,…}` for a spreadsheet document,
    /// `{10,…}` for a picture, and so on -- one structure per kind.
    pub(crate) payload: &'a str,
    /// The field's own event bindings -- see [`format_native_events`].
    pub(crate) events: &'a str,
    pub(crate) context_menu: &'a str,
    /// `<Visible>`, on unless the field turns it off.
    pub(crate) visible: bool,
    /// The two format patterns and the two format strings.
    pub(crate) formats: [&'a str; 2],
    pub(crate) format_strings: [&'a str; 2],
    /// `<FixingInTable>`: `Left` or `Right`.
    pub(crate) fixing_in_table: Option<&'a str>,
    /// `<ToolTipRepresentation>`.
    pub(crate) tooltip_representation: Option<&'a str>,
    pub(crate) extended_tooltip: &'a str,
    /// `<GroupHorizontalAlign>` and `<GroupVerticalAlign>`.
    pub(crate) group_horizontal_align: Option<&'a str>,
    pub(crate) group_vertical_align: Option<&'a str>,
    /// Member 55, see [`native_display_importance`].
    pub(crate) display_importance: &'a str,
    /// The additions a field carries in its tail, `<n>` then the records:
    /// `0` for every field but a PDF document's, whose `<ViewStatusAddition>`
    /// is `1,{…}` (8 of 8).
    pub(crate) additions: &'a str,
    /// Member 58, the count of the items nested behind the shared layout,
    /// then the items: `0` for every field but a Gantt chart's, whose
    /// `<Table>` is `1,{55,…}` (form_schema.rs: 17 of the 20 stand items
    /// carry one, and slot 58 counts it on all 366 field records of their
    /// bodies).
    pub(crate) nested_items: &'a str,
}

/// The `DisplayImportance` XML attribute an item may carry, as every item
/// record stores it: absent 0, `VeryHigh` 1, `High` 2, `Usual` 3, `Low` 4,
/// `VeryLow` 5 -- pure over the button, field, decoration and group records
/// of BSP. A spelling outside those is refused.
pub(crate) fn native_display_importance(value: Option<&str>) -> Option<&'static str> {
    match value {
        None => Some("0"),
        Some("VeryHigh") => Some("1"),
        Some("High") => Some("2"),
        Some("Usual") => Some("3"),
        Some("Low") => Some("4"),
        Some("VeryLow") => Some("5"),
        Some(_) => None,
    }
}

impl Default for NativeFieldItem<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            kind: 2,
            functional_options: None,
            name: "",
            title_location: None,
            title_height: None,
            title: "{1,0}",
            tooltip_title: "{1,0}",
            data_path: "{0}",
            footer_data_path: "{0}",
            enabled: true,
            read_only: false,
            skip_on_input: None,
            default_item: false,
            warning_on_edit: None,
            header_title: "{1,0}",
            footer_title: "{1,0}",
            show_in_header: true,
            show_in_footer: true,
            cell_hyperlink: false,
            horizontal_align: None,
            header_horizontal_align: None,
            footer_horizontal_align: None,
            vertical_align: None,
            edit_mode: None,
            auto_cell_height: false,
            header_picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            footer_picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            appearance: [
                "{3,4,{0}}",
                "{7,3,0,1,100}",
                "{3,4,{0}}",
                "{3,4,{0}}",
                "{3,4,{0}}",
                "{7,3,0,1,100}",
                "{0}",
            ],
            picture_index: "{0,0,0}",
            payload: "",
            events: "{0,1,0}",
            context_menu: "",
            visible: true,
            formats: ["{\"Pattern\"}", "{\"Pattern\"}"],
            format_strings: ["\"\"", "\"\""],
            fixing_in_table: None,
            tooltip_representation: None,
            extended_tooltip: "",
            group_horizontal_align: None,
            group_vertical_align: None,
            display_importance: "0",
            additions: "0",
            nested_items: "0",
        }
    }
}

/// Member 5 of a `{37,…}` record, by the element that names the field. The
/// correspondence is exact over the 122 767 field records of ERP УХ: every
/// `LabelField` writes 1, every `InputField` 2, and so on, with no kind
/// sharing a number.
pub(crate) fn native_field_kind(tag: &str) -> Option<u8> {
    Some(match tag {
        "LabelField" => 1,
        "InputField" => 2,
        "CheckBoxField" => 3,
        "PictureField" => 4,
        "RadioButtonField" => 5,
        "SpreadSheetDocumentField" => 6,
        "TextDocumentField" => 7,
        "CalendarField" => 8,
        "ProgressBarField" => 9,
        "TrackBarField" => 10,
        "ChartField" => 11,
        "GanttChartField" => 12,
        "GraphicalSchemaField" => 14,
        "HTMLDocumentField" => 15,
        "FormattedDocumentField" => 17,
        "PDFDocumentField" => 20,
        _ => return None,
    })
}

/// How a tri-state boolean reads: 2 when the item names neither value.
fn native_tristate(value: Option<bool>) -> &'static str {
    match value {
        None => "2",
        Some(true) => "1",
        Some(false) => "0",
    }
}

/// The `{37,…}` record of a field.
pub(crate) fn format_field_item(item: &NativeFieldItem<'_>) -> Option<String> {
    let options = match item.functional_options {
        Some(block) => format!("1,{block}"),
        None => "0".to_string(),
    };
    let title_location = root_code(
        item.title_location,
        &[
            ("None", "0"),
            ("Auto", "1"),
            ("Left", "2"),
            ("Top", "3"),
            ("Right", "4"),
            ("Bottom", "5"),
        ],
        "1",
    )?;
    let horizontal = root_code(
        item.horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let header_horizontal = root_code(
        item.header_horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2"), ("Auto", "3")],
        "0",
    )?;
    let footer_horizontal = root_code(
        item.footer_horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let vertical = root_code(
        item.vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    let edit_mode = root_code(
        item.edit_mode,
        &[("Directly", "0"), ("Enter", "1"), ("EnterOnInput", "2")],
        "1",
    )?;
    let warning = root_code(
        item.warning_on_edit,
        &[("Show", "0"), ("DontShow", "1")],
        "2",
    )?;
    let fixing = root_code(
        item.fixing_in_table,
        &[("None", "0"), ("Left", "1"), ("Right", "2")],
        "0",
    )?;
    let tooltip_representation = root_code(
        item.tooltip_representation,
        &[
            ("Auto", "0"),
            ("None", "1"),
            ("Balloon", "2"),
            ("Button", "3"),
            ("ShowAuto", "4"),
            ("ShowTop", "5"),
            ("ShowLeft", "6"),
            ("ShowBottom", "7"),
            ("ShowRight", "8"),
        ],
        "0",
    )?;
    let group_horizontal = root_code(
        item.group_horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let group_vertical = root_code(
        item.group_vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    Some(format!(
        "{{37,{{{id},{ns}}},0,0,{options},{kind},{name},{title_location},{title_height},\
         {title},{tooltip_title},{data_path},{footer_data_path},{enabled},{read_only},\
         {skip_on_input},{default_item},{warning},{header_title},{footer_title},\
         {show_in_header},{show_in_footer},{cell_hyperlink},{horizontal},{header_horizontal},\
         {footer_horizontal},{edit_mode},{vertical},{auto_cell_height},{header_picture},\
         {footer_picture},{a0},{a1},{a2},{a3},{a4},{a5},{picture_index},1,{payload},{events},1,\
         {context_menu},{visible},{format_one},{format_two},{string_one},{string_two},\
         {appearance_tail},{fixing},{tooltip_representation},1,{extended_tooltip},\
         {group_horizontal},{group_vertical},{display_importance},0,{additions},{nested_items}}}",
        id = item.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = item.kind,
        name = quoted(item.name),
        title_height = item.title_height.unwrap_or("0"),
        title = item.title,
        tooltip_title = item.tooltip_title,
        data_path = item.data_path,
        footer_data_path = item.footer_data_path,
        enabled = u8::from(item.enabled),
        read_only = u8::from(item.read_only),
        skip_on_input = native_tristate(item.skip_on_input),
        default_item = u8::from(item.default_item),
        header_title = item.header_title,
        footer_title = item.footer_title,
        show_in_header = u8::from(item.show_in_header),
        show_in_footer = u8::from(item.show_in_footer),
        cell_hyperlink = u8::from(item.cell_hyperlink),
        auto_cell_height = u8::from(item.auto_cell_height),
        header_picture = item.header_picture,
        footer_picture = item.footer_picture,
        a0 = item.appearance[0],
        a1 = item.appearance[1],
        a2 = item.appearance[2],
        a3 = item.appearance[3],
        a4 = item.appearance[4],
        a5 = item.appearance[5],
        appearance_tail = item.appearance[6],
        picture_index = item.picture_index,
        payload = item.payload,
        events = item.events,
        context_menu = item.context_menu,
        visible = u8::from(item.visible),
        format_one = item.formats[0],
        format_two = item.formats[1],
        string_one = item.format_strings[0],
        string_two = item.format_strings[1],
        extended_tooltip = item.extended_tooltip,
        display_importance = item.display_importance,
        additions = item.additions,
        nested_items = item.nested_items,
    ))
}

/// The `{11,…}` payload of a label or input field that carries no appearance
/// of its own, with the one slot that separates a field the user edits from
/// one that only shows.
pub(crate) fn format_plain_field_payload(editable: bool) -> String {
    format_label_payload(&NativeLabelPayload::plain(editable))
}

/// The `{11,…}` payload of a label field, member by member.
///
/// Five of its twenty members carry an XML property. They were read off the
/// corpus and then measured against it: a candidate that fills these five and
/// copies the rest reproduces **all 37 078** label payloads of every ERP УХ
/// form body exactly, with no record left over. The other members are the
/// appearance blocks and the event bindings, which are shapes of their own.
pub(crate) struct NativeLabelPayload<'a> {
    /// Slot 1, `0` when the item names no width.
    pub(crate) width: &'a str,
    /// Slot 2, `0` when it names no height.
    pub(crate) height: &'a str,
    /// Slot 3: `false` -> 0, `true` -> 1, absent -> 2.
    pub(crate) horizontal_stretch: Option<bool>,
    /// Slot 4, `<VerticalStretch>`, read the same way.
    pub(crate) vertical_stretch: Option<bool>,
    /// Slot 5, `<MarkNegatives>`, read the same way.
    pub(crate) mark_negatives: Option<bool>,
    /// Slot 6, `{1,0}` when the item names no format.
    pub(crate) format: &'a str,
    /// Slot 7 is `<Hiperlink>` -- the corpus spells it with one `p` -- on when
    /// the item says so and off otherwise. The partition test named it over
    /// all 37 005 label payloads; it was a guess before.
    pub(crate) hyperlink: bool,
    /// Slot 8, the text colour.
    pub(crate) text_color: &'a str,
    /// Slot 9, the background colour.
    pub(crate) back_color: &'a str,
    /// Slot 10, the font.
    pub(crate) font: &'a str,
    /// Slot 11, `<PasswordMode>`, tri-state like the stretches.
    pub(crate) password_mode: Option<bool>,
    /// Slot 12, the item's own event bindings.
    pub(crate) events: &'a str,
    /// Slot 13, `<BorderColor>`.
    pub(crate) border_color: &'a str,
    /// Slot 14, the `<Border>`.
    pub(crate) border: &'a str,
    /// Slot 15: `0` exactly when the item says `AutoMaxWidth` is false.
    pub(crate) auto_max_width: bool,
    /// Slot 16, `0` when the item names no maximum width.
    pub(crate) max_width: &'a str,
    /// Slot 18 and slot 19, the height's pair of the two above.
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: &'a str,
}

impl NativeLabelPayload<'_> {
    /// What a label that names no property of its own carries.
    pub(crate) const fn plain(hyperlink: bool) -> Self {
        Self {
            width: "0",
            height: "0",
            horizontal_stretch: None,
            vertical_stretch: None,
            mark_negatives: None,
            format: "{1,0}",
            hyperlink,
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            password_mode: None,
            events: "{0,1,0}",
            border_color: "{3,4,{0}}",
            border: "{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}",
            auto_max_width: true,
            max_width: "0",
            auto_max_height: true,
            max_height: "0",
        }
    }
}

pub(crate) fn format_label_payload(payload: &NativeLabelPayload<'_>) -> String {
    let tristate = |value: Option<bool>| match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{11,{width},{height},{stretch},{vertical},{mark_negatives},{format},{hyperlink},\
         {text_color},{back_color},{font},{password_mode},{events},{border_color},\
         {border},{auto_max_width},{max_width},0,{auto_max_height},\
         {max_height}}}",
        width = payload.width,
        height = payload.height,
        stretch = tristate(payload.horizontal_stretch),
        vertical = tristate(payload.vertical_stretch),
        mark_negatives = tristate(payload.mark_negatives),
        format = payload.format,
        hyperlink = u8::from(payload.hyperlink),
        auto_max_height = u8::from(payload.auto_max_height),
        max_height = payload.max_height,
        text_color = payload.text_color,
        back_color = payload.back_color,
        font = payload.font,
        events = payload.events,
        password_mode = tristate(payload.password_mode),
        border_color = payload.border_color,
        border = payload.border,
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
    )
}

/// Slot 62 of an input payload once the item names any of its four
/// properties: member 2 is `<AllowInputEmptyMultipleValues>` (absent 0,
/// `true` 1), member 4 is `<ShowCheckBoxesInDropList>` (absent 2, `false` 0,
/// `true` 1), member 9 `<MultipleValueDataPath>` and member 15
/// `<MultipleValuePresentDataPath>`, each `{1,{<column id>}}` or `{0}`. The
/// rest is constant over the 5 such fields of both corpora.
pub(crate) fn format_input_drop_list_settings(
    allow_empty_multiple_values: bool,
    show_check_boxes: Option<bool>,
    value_path: &str,
    present_path: &str,
) -> String {
    let check = match show_check_boxes {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{1,2,{allow_empty},0,{check},{{7,3,0,1,100}},{{3,4,{{0}}}},{{3,4,{{0}}}},\
         {{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{value_path},\"\",{{\"Pattern\"}},{{0}},\"\",\
         {{\"Pattern\"}},{present_path},\"\",{{\"Pattern\"}},0,0}}",
        allow_empty = u8::from(allow_empty_multiple_values),
    )
}

/// The `{3,…}` payload of a `<GraphicalSchemaField>` (rt-fields2.md §2, 11 of
/// 11): width and height default 50 and 10, `<Output>` absent 0 and `Enable`
/// 1, `<Edit>` 1 unless `false`, then the events and constants.
pub(crate) fn format_graphical_schema_payload(
    width: &str,
    height: &str,
    output: Option<&str>,
    edit: bool,
    events: &str,
) -> Option<String> {
    let output = match output {
        None => "0",
        Some("Enable") => "1",
        Some(_) => return None,
    };
    Some(format!(
        "{{3,{width},{height},{output},{edit},{{3,4,{{0}}}},{events},1,0,0,1,0,1,1}}",
        edit = u8::from(edit),
    ))
}

/// The `{1,…}` payload of a `<ChartField>` (rt-fields2.md §3, 9 of 9).
pub(crate) fn format_chart_payload(
    width: &str,
    height: &str,
    horizontal_stretch: bool,
    vertical_stretch: bool,
    events: &str,
    max_height: &str,
) -> String {
    format!(
        "{{1,{width},{height},{horizontal},{vertical},{events},1,0,0,1,{max_height}}}",
        horizontal = u8::from(horizontal_stretch),
        vertical = u8::from(vertical_stretch),
    )
}

/// The `{3,…}` payload of a `<GanttChartField>`: width and height default 50
/// and 10, the two stretch flags 1 unless the field turns them off, the
/// events, then ten members that hold one value each over the whole stand
/// (`form_schema::FormSpecialFieldSchema::gantt_dimension` and
/// `gantt_stretch` read the first four; 18 of 18 ERP УХ records spell the
/// bag at revision 3).
pub(crate) fn format_gantt_chart_payload(
    width: &str,
    height: &str,
    horizontal_stretch: bool,
    vertical_stretch: bool,
    events: &str,
) -> String {
    format!(
        "{{3,{width},{height},{horizontal},{vertical},{events},1,0,0,1,0,0,0,0,2,2}}",
        horizontal = u8::from(horizontal_stretch),
        vertical = u8::from(vertical_stretch),
    )
}

/// The `{1,…}` payload of a `<PDFDocumentField>` (rt-fields2.md §4, 8 of 8):
/// only the size varies.
pub(crate) fn format_pdf_document_payload(width: &str, height: &str) -> String {
    format!("{{1,{width},{height},{{3,4,{{0}}}},0,{{0,1,0}},1,0,0,1,0,1,1,0}}")
}

/// The `{36,…}` payload of an input field, member by member.
///
/// Seven of its sixty-six members carry an XML property. Measured the same way
/// as the label: a candidate that fills these seven and copies the rest
/// reproduces 69 452 of the 69 455 input payloads of every ERP УХ form body
/// exactly, and the three it does not are a mask that carries a `;`, which the
/// measurement's own property separator truncated -- not a rule this writer
/// gets wrong.
pub(crate) struct NativeInputPayload<'a> {
    /// Slot 2, `0` when the item names no width.
    pub(crate) width: &'a str,
    /// Slot 3, `0` when it names no height.
    pub(crate) height: &'a str,
    /// Slot 4: `false` -> 0, `true` -> 1, absent -> 2.
    pub(crate) horizontal_stretch: Option<bool>,
    /// Slot 5, the same three ways for `VerticalStretch`.
    pub(crate) vertical_stretch: Option<bool>,
    /// Slot 6, `<Wrap>`, on unless the item turns it off.
    pub(crate) wrap: bool,
    /// Slots 7 to 15, each a tri-state read the same way as the stretches:
    /// `<PasswordMode>`, `<MultiLine>`, `<ExtendedEdit>`, `<MarkNegatives>`,
    /// `<ChoiceListButton>`, `<ChoiceButton>`, `<ClearButton>`,
    /// `<SpinButton>` and `<OpenButton>`.
    pub(crate) password_mode: Option<bool>,
    pub(crate) multi_line: Option<bool>,
    pub(crate) extended_edit: Option<bool>,
    pub(crate) mark_negatives: Option<bool>,
    pub(crate) choice_list_button: Option<bool>,
    pub(crate) choice_button: Option<bool>,
    pub(crate) clear_button: Option<bool>,
    pub(crate) spin_button: Option<bool>,
    pub(crate) open_button: Option<bool>,
    /// Slots 16 and 17, `<MinValue>` and `<MaxValue>` as typed values,
    /// `{"U"}` when the item names neither.
    pub(crate) min_value: &'a str,
    pub(crate) max_value: &'a str,
    /// Slot 19, `<ListChoiceMode>`.
    pub(crate) list_choice_mode: bool,
    /// Slot 20, the item's picture.
    pub(crate) picture: &'a str,
    /// Slots 21 and 22, `<ChoiceListHeight>` and `<DropListWidth>`.
    pub(crate) choice_list_height: &'a str,
    pub(crate) drop_list_width: &'a str,
    /// Slot 23, `<QuickChoice>`.
    pub(crate) quick_choice: Option<bool>,
    /// Slot 24, `<ChoiceFoldersAndItems>`: `Items` 0, `Folders` 1,
    /// `FoldersAndItems` 2, and 3 when the item names none.
    pub(crate) choice_folders_and_items: Option<&'a str>,
    /// Slot 25, the uuid of the `<ChoiceForm>`, which only the configuration
    /// can give.
    pub(crate) choice_form: &'a str,
    /// Slot 28, `<AutoChoiceIncomplete>`.
    pub(crate) auto_choice_incomplete: Option<bool>,
    /// Slot 31, `<AutoMarkIncomplete>`.
    pub(crate) auto_mark_incomplete: Option<bool>,
    /// Slot 32, `<ChooseType>`, on unless the item turns it off.
    pub(crate) choose_type: bool,
    /// Slot 33, `<IncompleteChoiceMode>`: `OnActivate` 1.
    pub(crate) incomplete_choice_mode: Option<&'a str>,
    /// Slot 41, `<TextEdit>`, on unless the item turns it off.
    pub(crate) text_edit: bool,
    /// Slot 43, `<EditTextUpdate>`: `DontUse` 1, `OnValueChange` 2,
    /// `Always` 3.
    pub(crate) edit_text_update: Option<&'a str>,
    /// Slot 44, `<InputHint>`, a localized string, `{1,0}` by default.
    pub(crate) input_hint: &'a str,
    /// Slot 1, `<ChoiceList>`, `{3,0}` by default.
    pub(crate) choice_list: &'a str,
    /// Slots 26 and 64, `<ChoiceParameterLinks>`, `{5006,0}` and `{5007,0}`.
    pub(crate) choice_parameter_links: &'a str,
    pub(crate) choice_parameter_links_again: &'a str,
    /// Slot 42, `<TypeLink>`, `{3,0,0}` by default.
    pub(crate) type_link: &'a str,
    /// Slot 27, `<ChoiceParameters>`, `{0,0}` by default.
    pub(crate) choice_parameters: &'a str,
    /// Slot 34, `<AvailableTypes>` as a type pattern, `{"Pattern"}` by default.
    pub(crate) available_types: &'a str,
    /// Slots 55 to 60: `<AutoShowClearButtonMode>`, `<AutoShowOpenButtonMode>`,
    /// `<AutoCorrectionOnTextInput>`, `<SpellCheckingOnTextInput>`, a constant,
    /// `<SpecialTextInputMode>` -- already coded.
    pub(crate) text_input_tail: [&'a str; 6],
    /// Slot 45, `<CreateButton>`.
    pub(crate) create_button: Option<bool>,
    /// Slot 46, `<ChoiceButtonRepresentation>`: `ShowInDropList` 1,
    /// `ShowInDropListAndInInputField` 2, `ShowInInputField` 3.
    pub(crate) choice_button_representation: Option<&'a str>,
    /// Slot 47, `<DropListButton>`.
    pub(crate) drop_list_button: Option<bool>,
    /// Slot 48, `<ChoiceHistoryOnInput>`: `DontUse` 1.
    pub(crate) choice_history_on_input: Option<&'a str>,
    /// Slots 52, 53 and 54: `<AutoMaxHeight>`, `<MaxHeight>` and
    /// `<HeightControlVariant>`.
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: &'a str,
    pub(crate) height_control_variant: Option<&'a str>,
    /// Slot 35, `<TypeDomainEnabled>`: absent 1, `false` 0. A bare literal
    /// until now, and the corpus contradicts it on 25 records.
    pub(crate) type_domain_enabled: bool,
    /// Slot 65, `<ExtendedEditMultipleValues>`: absent 0, `true` 1. The same,
    /// on 3 354.
    pub(crate) extended_edit_multiple_values: bool,
    /// Slot 62, the drop-list settings: `{0}` unless the item names
    /// `<ShowCheckBoxesInDropList>` or a multiple-value data path -- see
    /// [`format_input_drop_list_settings`].
    pub(crate) drop_list_settings: &'a str,
    /// Slot 18, the input mask, `""` when the item names none.
    pub(crate) mask: &'a str,
    /// Slot 49: `0` exactly when the item says `AutoMaxWidth` is false.
    pub(crate) auto_max_width: bool,
    /// Slot 50, `0` when the item names no maximum width.
    pub(crate) max_width: &'a str,
    /// Slot 29, the format, `{1,0}` by default.
    pub(crate) format: &'a str,
    /// Slot 30, the edit format, `{1,0}` by default.
    pub(crate) edit_format: &'a str,
    /// Slot 36, the item's own event bindings.
    pub(crate) events: &'a str,
    /// Slots 37, 38 and 39: the text colour, the background colour and the
    /// border, `{3,4,{0}}` by default.
    pub(crate) text_color: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
    /// Slot 40, the font.
    pub(crate) font: &'a str,
}

impl NativeInputPayload<'_> {
    /// What an input field that names no property of its own carries -- the
    /// payload 24 361 of the corpus's records carry unchanged.
    pub(crate) const fn plain() -> Self {
        Self {
            width: "0",
            height: "0",
            horizontal_stretch: None,
            vertical_stretch: None,
            wrap: true,
            password_mode: None,
            multi_line: None,
            extended_edit: None,
            mark_negatives: None,
            choice_list_button: None,
            choice_button: None,
            clear_button: None,
            spin_button: None,
            open_button: None,
            min_value: "{\"U\"}",
            max_value: "{\"U\"}",
            list_choice_mode: false,
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            choice_list_height: "0",
            drop_list_width: "0",
            quick_choice: None,
            choice_folders_and_items: None,
            choice_form: "00000000-0000-0000-0000-000000000000",
            auto_choice_incomplete: None,
            auto_mark_incomplete: None,
            choose_type: true,
            incomplete_choice_mode: None,
            text_edit: true,
            edit_text_update: None,
            input_hint: "{1,0}",
            choice_list: "{3,0}",
            choice_parameter_links: "{5006,0}",
            choice_parameter_links_again: "{5007,0}",
            type_link: "{3,0,0}",
            choice_parameters: "{0,0}",
            available_types: "{\"Pattern\"}",
            text_input_tail: ["0", "0", "0", "0", "0", "0"],
            create_button: None,
            choice_button_representation: None,
            drop_list_button: None,
            choice_history_on_input: None,
            auto_max_height: true,
            max_height: "0",
            height_control_variant: None,
            type_domain_enabled: true,
            extended_edit_multiple_values: false,
            drop_list_settings: "{0}",
            mask: "",
            auto_max_width: true,
            max_width: "0",
            format: "{1,0}",
            edit_format: "{1,0}",
            events: "{0,1,0}",
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_input_payload(payload: &NativeInputPayload<'_>) -> Option<String> {
    let tristate = |value: Option<bool>| match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    let folders = root_code(
        payload.choice_folders_and_items,
        &[("Items", "0"), ("Folders", "1"), ("FoldersAndItems", "2")],
        "3",
    )?;
    let incomplete = root_code(
        payload.incomplete_choice_mode,
        &[("OnActivate", "1")],
        "0",
    )?;
    let edit_text_update = root_code(
        payload.edit_text_update,
        &[("DontUse", "1"), ("OnValueChange", "2"), ("Always", "3")],
        "0",
    )?;
    let choice_representation = root_code(
        payload.choice_button_representation,
        &[
            ("ShowInDropList", "1"),
            ("ShowInDropListAndInInputField", "2"),
            ("ShowInInputField", "3"),
        ],
        "0",
    )?;
    let history = root_code(payload.choice_history_on_input, &[("DontUse", "1")], "0")?;
    let height_variant = root_code(
        payload.height_control_variant,
        &[
            ("UseHeightInFormRows", "1"),
            ("UseContentHeight", "2"),
            ("UseHeightInTableRows", "3"),
        ],
        "0",
    )?;
    Some(format!(
        "{{36,{choice_list},{width},{height},{horizontal},{vertical},{wrap},{password},{multi_line},\
         {extended_edit},{mark_negatives},{choice_list_button},{choice_button},{clear_button},\
         {spin_button},{open_button},{min_value},{max_value},{mask},{list_choice_mode},\
         {picture},{choice_list_height},{drop_list_width},{quick_choice},{folders},\
         {choice_form},{links},{choice_parameters},{auto_choice_incomplete},{format},{edit_format},\
         {auto_mark_incomplete},{choose_type},{incomplete},{available_types},{type_domain},{events},\
         {text_color},{back_color},{border_color},{font},{text_edit},{type_link},\
         {edit_text_update},{input_hint},{create_button},{choice_representation},\
         {drop_list_button},{history},{auto_max_width},{max_width},0,{auto_max_height},\
         {max_height},{height_variant},{tail0},{tail1},{tail2},{tail3},{tail4},{tail5},0,{drop_list_settings},0,\
         {links_again},{multiple_values}}}",
        width = payload.width,
        height = payload.height,
        type_domain = u8::from(payload.type_domain_enabled),
        multiple_values = u8::from(payload.extended_edit_multiple_values),
        drop_list_settings = payload.drop_list_settings,
        horizontal = tristate(payload.horizontal_stretch),
        vertical = tristate(payload.vertical_stretch),
        wrap = u8::from(payload.wrap),
        password = tristate(payload.password_mode),
        multi_line = tristate(payload.multi_line),
        extended_edit = tristate(payload.extended_edit),
        mark_negatives = tristate(payload.mark_negatives),
        choice_list_button = tristate(payload.choice_list_button),
        choice_button = tristate(payload.choice_button),
        clear_button = tristate(payload.clear_button),
        spin_button = tristate(payload.spin_button),
        open_button = tristate(payload.open_button),
        min_value = payload.min_value,
        max_value = payload.max_value,
        mask = quoted(payload.mask),
        list_choice_mode = u8::from(payload.list_choice_mode),
        picture = payload.picture,
        choice_list_height = payload.choice_list_height,
        drop_list_width = payload.drop_list_width,
        quick_choice = tristate(payload.quick_choice),
        choice_form = payload.choice_form,
        auto_choice_incomplete = tristate(payload.auto_choice_incomplete),
        format = payload.format,
        edit_format = payload.edit_format,
        auto_mark_incomplete = tristate(payload.auto_mark_incomplete),
        choose_type = u8::from(payload.choose_type),
        events = payload.events,
        text_color = payload.text_color,
        back_color = payload.back_color,
        border_color = payload.border_color,
        font = payload.font,
        text_edit = u8::from(payload.text_edit),
        input_hint = payload.input_hint,
        choice_list = payload.choice_list,
        links = payload.choice_parameter_links,
        links_again = payload.choice_parameter_links_again,
        type_link = payload.type_link,
        choice_parameters = payload.choice_parameters,
        available_types = payload.available_types,
        tail0 = payload.text_input_tail[0],
        tail1 = payload.text_input_tail[1],
        tail2 = payload.text_input_tail[2],
        tail3 = payload.text_input_tail[3],
        tail4 = payload.text_input_tail[4],
        tail5 = payload.text_input_tail[5],
        create_button = tristate(payload.create_button),
        drop_list_button = tristate(payload.drop_list_button),
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
        auto_max_height = u8::from(payload.auto_max_height),
        max_height = payload.max_height,
    ))
}

/// The `{11,…}` payload of a check box field -- the same wrapper a label
/// carries, with thirteen members instead of twenty.
///
/// Eight of them carry an XML property, named by the partition test over all
/// 10 385 check boxes of ERP УХ. `<CheckBoxType>` is written twice: slot 4
/// tells `CheckBox` and `Tumbler` apart, and slot 12 also tells `Switcher`
/// from `Auto`.
pub(crate) struct NativeCheckBoxPayload<'a> {
    /// Slot 1, `<ThreeState>`.
    pub(crate) three_state: bool,
    /// Slots 4 and 12.
    pub(crate) check_box_type: Option<&'a str>,
    /// Slot 2, the text colour.
    pub(crate) text_color: &'a str,
    /// Slot 3, the background colour.
    pub(crate) back_color: &'a str,
    /// Slot 5, the format.
    pub(crate) format: &'a str,
    /// Slot 6, the border.
    pub(crate) border_color: &'a str,
    /// Slot 7, the font.
    pub(crate) font: &'a str,
    /// Slots 8, 9 and 10: `<ItemTitleHeight>`, `<ItemWidth>` and
    /// `<ItemHeight>`, 0 when the field names none.
    pub(crate) item_title_height: &'a str,
    pub(crate) item_width: &'a str,
    pub(crate) item_height: &'a str,
    /// Slot 11, `<EqualItemsWidth>`: `false` 0, `true` 1, absent 2.
    pub(crate) equal_items_width: Option<&'a str>,
}

impl NativeCheckBoxPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            three_state: false,
            check_box_type: None,
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            format: "{1,0}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            item_title_height: "0",
            item_width: "0",
            item_height: "0",
            equal_items_width: None,
        }
    }
}

pub(crate) fn format_check_box_payload(payload: &NativeCheckBoxPayload<'_>) -> Option<String> {
    let kind = root_code(
        payload.check_box_type,
        &[
            ("Auto", "0"),
            ("CheckBox", "1"),
            ("Tumbler", "2"),
            ("Switcher", "0"),
        ],
        "0",
    )?;
    let tail = root_code(
        payload.check_box_type,
        &[
            ("Auto", "0"),
            ("CheckBox", "1"),
            ("Tumbler", "2"),
            ("Switcher", "3"),
        ],
        "0",
    )?;
    let equal = root_code(
        payload.equal_items_width,
        &[("false", "0"), ("true", "1")],
        "2",
    )?;
    Some(format!(
        "{{11,{three_state},{text_color},{back_color},{kind},{format},{border_color},{font},{item_title_height},{item_width},{item_height},{equal},{tail}}}",
        three_state = u8::from(payload.three_state),
        text_color = payload.text_color,
        back_color = payload.back_color,
        format = payload.format,
        border_color = payload.border_color,
        font = payload.font,
        item_title_height = payload.item_title_height,
        item_width = payload.item_width,
        item_height = payload.item_height,
    ))
}

/// The `{8,…}` payload of a radio-button field.
///
/// Six of its twelve members carry an XML property, named by the partition
/// test over all 2 282 radio buttons of ERP УХ.
pub(crate) struct NativeRadioButtonPayload<'a> {
    /// Slot 2, `<ColumnsCount>`, 0 when the field names none.
    pub(crate) columns: &'a str,
    /// Slot 7, `<RadioButtonType>`.
    pub(crate) radio_button_type: Option<&'a str>,
    /// Slot 1, the choice list the field offers.
    pub(crate) choice_list: &'a str,
    /// Slot 3, the text colour; slot 5, the background; slot 8, the border.
    pub(crate) text_color: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
    /// Slot 4, the font.
    pub(crate) font: &'a str,
    /// Slots 6, 9 and 10: `<ItemHeight>`, `<ItemTitleHeight>` and
    /// `<ItemWidth>`, 0 when the field names none.
    pub(crate) item_height: &'a str,
    pub(crate) item_title_height: &'a str,
    pub(crate) item_width: &'a str,
    /// Slot 11, `<EqualColumnsWidth>`: `false` 0, `true` 1, absent 2.
    pub(crate) equal_columns_width: Option<&'a str>,
}

impl NativeRadioButtonPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            columns: "0",
            radio_button_type: None,
            choice_list: "{3,0}",
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            item_height: "0",
            item_title_height: "0",
            item_width: "0",
            equal_columns_width: None,
        }
    }
}

/// The value member of every design-time choice-list item, constant in all
/// 4 769 items of both corpora.
const CHOICE_LIST_VALUE_UUID: &str = "0e704aa2-07bd-48b9-8223-a0212c4d5fc2";

/// The picture member of a choice-list item -- one constant, because no item
/// of either corpus carries a `<Picture>`.
const CHOICE_LIST_PICTURE: &str = "{0,{4,0,{0},\"\",-1,-1,1,0,\"\"}}";

/// `ent:AccountType`, whose three spellings are an ordinal under this uuid:
/// `Active` 0, `Passive` 1, `ActivePassive` 2, identical in both corpora.
const CHOICE_LIST_ACCOUNT_TYPE_UUID: &str = "872f7198-7083-4e3e-b57e-a2a9802c769e";

/// The `{3,…}` choice list a field offers.
///
/// ```text
/// {3, N, <presentation>, <value>, … , <picture>, <picture>, …}
/// ```
///
/// `2 + 3N` members: N presentation/value pairs and then N pictures. Measured
/// over the 2 637 fields of both corpora that carry a `<ChoiceList>` and the
/// 4 769 items in them -- the outer `<xr:Presentation>` is `""` in every one,
/// and the picture is the same constant in every one. Absence is a pure
/// partition: a field with no `<ChoiceList>` stores `{3,0}` in all 110
/// records that lack one, and no record with a list stores it.
pub(crate) fn format_native_choice_list(values: &[String]) -> String {
    if values.is_empty() {
        return "{3,0}".to_string();
    }
    let mut out = format!("{{3,{}", values.len());
    for value in values {
        out.push_str(",\"\",");
        out.push_str(value);
    }
    for _ in values {
        out.push(',');
        out.push_str(CHOICE_LIST_PICTURE);
    }
    out.push('}');
    out
}

/// The literal a choice-list item stores, by the `xsi:type` of its inner
/// `<Value>`.
pub(crate) enum NativeChoiceListLiteral<'a> {
    /// `xs:decimal` -- 2 280 items.
    Number(&'a str),
    /// `xs:string` -- 1 047 items.
    Text(&'a str),
    /// `xs:boolean` -- 247 values in 124 input field lists.
    Boolean(bool),
    /// A literal already spelled -- a fixed array, a date.
    Raw(&'a str),
    /// `ent:AccountType` -- 6 items, the ordinal of its three spellings.
    AccountType(u8),
    /// `xr:DesignTimeRef`, and an empty `<Value/>` with no type at all.
    Undefined,
}

impl NativeChoiceListLiteral<'_> {
    fn spelled(&self) -> String {
        match self {
            Self::Number(value) => format!("{{\"N\",{value}}}"),
            Self::Text(value) => format!("{{\"S\",{}}}", quoted(value)),
            Self::Boolean(value) => format!("{{\"B\",{}}}", u8::from(*value)),
            Self::Raw(value) => (*value).to_string(),
            Self::AccountType(ordinal) => {
                format!("{{\"#\",{CHOICE_LIST_ACCOUNT_TYPE_UUID},{ordinal}}}")
            }
            Self::Undefined => "{\"U\"}".to_string(),
        }
    }
}

/// One item's value:
/// `{"#",0e704aa2-…,{0,B,<literal>,<type id>,<value id>,<title>}}`.
///
/// `B` is a pure partition over all 4 769 items: an `xsi:type` of
/// `xr:DesignTimeRef` writes 0 and every other spelling writes 1. The two
/// uuids are the owner's Ref `<xr:TypeId>` and the referenced value's id for
/// a reference, and zero for a literal; the title is the item's **inner**
/// `<Presentation>`, not its outer one.
pub(crate) fn format_native_choice_list_value(
    design_time_ref: bool,
    literal: &NativeChoiceListLiteral<'_>,
    type_id: &str,
    value_id: &str,
    title: &str,
) -> String {
    format!(
        "{{\"#\",{CHOICE_LIST_VALUE_UUID},{{0,{flag},{literal},{type_id},{value_id},{title}}}}}",
        flag = u8::from(!design_time_ref),
        literal = literal.spelled(),
    )
}

pub(crate) fn format_radio_button_payload(
    payload: &NativeRadioButtonPayload<'_>,
) -> Option<String> {
    let kind = root_code(
        payload.radio_button_type,
        &[("Auto", "0"), ("RadioButtons", "1"), ("Tumbler", "2")],
        "0",
    )?;
    let equal = root_code(
        payload.equal_columns_width,
        &[("false", "0"), ("true", "1")],
        "2",
    )?;
    Some(format!(
        "{{8,{choice_list},{columns},{text_color},{font},{back_color},{item_height},{kind},{border_color},{item_title_height},{item_width},{equal}}}",
        choice_list = payload.choice_list,
        columns = payload.columns,
        text_color = payload.text_color,
        font = payload.font,
        back_color = payload.back_color,
        item_height = payload.item_height,
        border_color = payload.border_color,
        item_title_height = payload.item_title_height,
        item_width = payload.item_width,
    ))
}

/// The `{10,…}` payload of a picture field.
///
/// Eleven of its twenty-four members carry an XML property. The partition test
/// over all 2 201 of the corpus named six the writer had been taking from its
/// caller: `<HorizontalStretch>` and `<VerticalStretch>` at slots 3 and 4,
/// `<Hiperlink>` at slot 8, `<AutoMaxWidth>` and `<AutoMaxHeight>` at 17 and
/// 20, and `<FileDragMode>` at 22.
///
/// Two of those read unlike their namesakes elsewhere. A picture's stretches
/// are **two**-valued, not three: absent writes 1, the same as `true`, where a
/// label's absent stretch writes 2. And `<FileDragMode>` is inverted the way
/// the table tail's is -- 1 when the field names none, 0 for `AsFile`.
pub(crate) struct NativePicturePayload<'a> {
    pub(crate) width: &'a str,
    pub(crate) height: &'a str,
    pub(crate) horizontal_stretch: bool,
    pub(crate) vertical_stretch: bool,
    /// `<PictureSize>`: `Stretch` 1, `Proportionally` 2, `AutoSize` 4,
    /// `AutoSizeIgnoreScale` 6, `ByFontSize` 7.
    pub(crate) picture_size: Option<&'a str>,
    pub(crate) hyperlink: bool,
    pub(crate) auto_max_width: bool,
    pub(crate) max_width: &'a str,
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: &'a str,
    /// `<FileDragMode>`, of which only `AsFile` is ever stored.
    pub(crate) file_drag_mode: Option<&'a str>,
    /// Slot 7, `<Zoomable>`, and slot 15, `<EnableDrag>`.
    pub(crate) zoomable: bool,
    pub(crate) enable_drag: bool,
    /// Already formatted -- the picture, the title, the colours, the font,
    /// the border and the events.
    pub(crate) picture: &'a str,
    pub(crate) title: &'a str,
    pub(crate) text_color: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) font: &'a str,
    pub(crate) border: &'a str,
    pub(crate) events: &'a str,
}

impl Default for NativePicturePayload<'_> {
    fn default() -> Self {
        Self {
            width: "0",
            height: "0",
            horizontal_stretch: true,
            vertical_stretch: true,
            picture_size: None,
            hyperlink: false,
            auto_max_width: true,
            max_width: "0",
            auto_max_height: true,
            max_height: "0",
            file_drag_mode: None,
            zoomable: false,
            enable_drag: false,
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            title: "{1,0}",
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            border: "{3,0,{0},1,1,0,48312c09-257f-4b29-b280-284dd89efc1e}",
            events: "{0,1,0}",
        }
    }
}

pub(crate) fn format_picture_payload(payload: &NativePicturePayload<'_>) -> Option<String> {
    let size = root_code(
        payload.picture_size,
        &[
            ("RealSize", "0"),
            ("Stretch", "1"),
            ("Proportionally", "2"),
            ("AutoSize", "4"),
            ("AutoSizeIgnoreScale", "6"),
            ("ByFontSize", "7"),
        ],
        "0",
    )?;
    // A picture that names no drag mode writes 1, not 0.
    let drag = root_code(payload.file_drag_mode, &[("AsFile", "0")], "1")?;
    Some(format!(
        "{{10,{width},{height},{horizontal},{vertical},{picture},{size},{zoomable},{hyperlink},{title},\
         {text_color},{back_color},{font},{border},0,{enable_drag},{events},{auto_max_width},\
         {max_width},0,{auto_max_height},{max_height},{drag},100}}",
        zoomable = u8::from(payload.zoomable),
        enable_drag = u8::from(payload.enable_drag),
        width = payload.width,
        height = payload.height,
        horizontal = u8::from(payload.horizontal_stretch),
        vertical = u8::from(payload.vertical_stretch),
        picture = payload.picture,
        hyperlink = u8::from(payload.hyperlink),
        title = payload.title,
        text_color = payload.text_color,
        back_color = payload.back_color,
        font = payload.font,
        border = payload.border,
        events = payload.events,
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
        auto_max_height = u8::from(payload.auto_max_height),
        max_height = payload.max_height,
    ))
}

/// The `{13,…}` payload of a spreadsheet document field.
///
/// Twenty-four of its thirty-two members carry an XML property, named by the
/// partition test over all 942 spreadsheet fields of ERP УХ. Three are
/// written twice under two codings: both scroll bars, where the later slot
/// tells "names none" from `true`, and `<SelectionShowMode>`, where the later
/// slot tells all four spellings apart.
pub(crate) struct NativeSpreadsheetPayload<'a> {
    /// Slots 1 and 2, `<Width>` and `<Height>`, 50 and 10 by default.
    pub(crate) width: &'a str,
    pub(crate) height: &'a str,
    /// Slots 3 and 4, the two stretches, on unless the field turns them off.
    pub(crate) horizontal_stretch: bool,
    pub(crate) vertical_stretch: bool,
    /// Slots 5 and 6, `<ShowGrid>` and `<ShowHeaders>`.
    pub(crate) show_grid: bool,
    pub(crate) show_headers: bool,
    /// Slots 7 and 28, and 8 and 29: the two scroll bars.
    pub(crate) vertical_scroll_bar: Option<&'a str>,
    pub(crate) horizontal_scroll_bar: Option<&'a str>,
    /// Slot 10, `<Protection>`.
    pub(crate) protection: bool,
    /// Slots 11 and 30, `<SelectionShowMode>`.
    pub(crate) selection_show_mode: Option<&'a str>,
    /// Slot 12, `<Output>`: `Enable` 1, `Disable` 2, absent 0.
    pub(crate) output: Option<&'a str>,
    /// Slots 13 and 14, `<Edit>` and `<ShowGroups>`.
    pub(crate) edit: bool,
    pub(crate) show_groups: bool,
    /// Slot 15, the border colour.
    pub(crate) border_color: &'a str,
    /// Slots 16 and 17, `<EnableStartDrag>` and `<EnableDrag>`.
    pub(crate) enable_start_drag: bool,
    pub(crate) enable_drag: bool,
    /// Slot 18, the field's events.
    pub(crate) events: &'a str,
    /// Slot 19, `<ViewScalingMode>`: `Normal` 1, absent 0.
    pub(crate) view_scaling_mode: Option<&'a str>,
    /// Slots 20, 21, 23 and 24: the two maxima and their flags.
    pub(crate) auto_max_width: bool,
    pub(crate) max_width: &'a str,
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: &'a str,
    /// Slots 25 and 26, `<ShowCellNames>` and `<ShowRowAndColumnNames>`.
    pub(crate) show_cell_names: bool,
    pub(crate) show_row_and_column_names: bool,
    /// Slot 31, `<DrawingSelectionShowMode>`: absent 2, `Show` 0 (1 of 1).
    pub(crate) drawing_selection_show_mode: Option<&'a str>,
}

impl NativeSpreadsheetPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            width: "50",
            height: "10",
            horizontal_stretch: true,
            vertical_stretch: true,
            show_grid: false,
            show_headers: false,
            vertical_scroll_bar: None,
            horizontal_scroll_bar: None,
            protection: false,
            selection_show_mode: None,
            output: None,
            edit: false,
            show_groups: true,
            border_color: "{3,4,{0}}",
            enable_start_drag: true,
            enable_drag: true,
            events: "{0,1,0}",
            view_scaling_mode: None,
            auto_max_width: true,
            max_width: "0",
            auto_max_height: true,
            max_height: "0",
            show_cell_names: false,
            show_row_and_column_names: false,
            drawing_selection_show_mode: None,
        }
    }
}

pub(crate) fn format_spreadsheet_payload(
    payload: &NativeSpreadsheetPayload<'_>,
) -> Option<String> {
    let bar = |value| root_code(value, &[("false", "0"), ("true", "1")], "1");
    let bar_tail = |value| root_code(value, &[("false", "0"), ("true", "1")], "2");
    let vertical = bar(payload.vertical_scroll_bar)?;
    let horizontal = bar(payload.horizontal_scroll_bar)?;
    let vertical_tail = bar_tail(payload.vertical_scroll_bar)?;
    let horizontal_tail = bar_tail(payload.horizontal_scroll_bar)?;
    let selection = root_code(
        payload.selection_show_mode,
        &[
            ("WhenActive", "0"),
            ("DontShow", "1"),
            ("WhenMultipleCellsSelected", "1"),
        ],
        "1",
    )?;
    let selection_tail = root_code(
        payload.selection_show_mode,
        &[
            ("WhenActive", "0"),
            ("DontShow", "2"),
            ("WhenMultipleCellsSelected", "3"),
        ],
        "1",
    )?;
    let output = root_code(payload.output, &[("Enable", "1"), ("Disable", "2")], "0")?;
    let scaling = root_code(payload.view_scaling_mode, &[("Normal", "1")], "0")?;
    let drawing_selection = root_code(payload.drawing_selection_show_mode, &[("Show", "0")], "2")?;
    Some(format!(
        "{{13,{width},{height},{horizontal_stretch},{vertical_stretch},{show_grid},{show_headers},{vertical},{horizontal},0,{protection},{selection},{output},{edit},{show_groups},{border_color},{enable_start_drag},{enable_drag},{events},{scaling},{auto_max_width},{max_width},0,{auto_max_height},{max_height},{show_cell_names},{show_row_and_column_names},0,{vertical_tail},{horizontal_tail},{selection_tail},{drawing_selection}}}",
        width = payload.width,
        height = payload.height,
        horizontal_stretch = u8::from(payload.horizontal_stretch),
        vertical_stretch = u8::from(payload.vertical_stretch),
        show_grid = u8::from(payload.show_grid),
        show_headers = u8::from(payload.show_headers),
        protection = u8::from(payload.protection),
        edit = u8::from(payload.edit),
        show_groups = u8::from(payload.show_groups),
        border_color = payload.border_color,
        enable_start_drag = u8::from(payload.enable_start_drag),
        enable_drag = u8::from(payload.enable_drag),
        events = payload.events,
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
        auto_max_height = u8::from(payload.auto_max_height),
        max_height = payload.max_height,
        show_cell_names = u8::from(payload.show_cell_names),
        show_row_and_column_names = u8::from(payload.show_row_and_column_names),
    ))
}

/// The `{3,…}`, `{5,…}` and `{1,…}` payloads of the HTML, text and formatted
/// document fields.
///
/// Two of their members carry an XML property -- the width and the height, at
/// slots 1 and 2 in all three, with 50 and 10 the values a field that names
/// neither carries. All 135, 94 and 47 records of the corpus are reproduced
/// exactly, first time.
pub(crate) fn format_document_payload(wrapper: u8, width: &str, height: &str, tail: &str) -> String {
    format!("{{{wrapper},{width},{height},{tail}}}")
}

/// The uuid a `<Button>` stores for `Form.Item.<item>.StandardCommand.<name>`,
/// by the target item's tag, whether it is bound to a dynamic list, and the
/// command's name. Measured over every item button of ERP УХ.
const ITEM_STANDARD_COMMAND_UUIDS: &[(&str, bool, &str, &str)] = &[
    ("FormattedDocumentField", false, "AlignCenter", "ab0ebc39-68ee-4034-b2f4-43eee55bd651"),
    ("FormattedDocumentField", false, "AlignJustify", "56ae90b6-588f-406e-919c-cc5cc7f86297"),
    ("FormattedDocumentField", false, "AlignLeft", "87ecfbdd-8e2b-4ba2-a315-0897020f382f"),
    ("FormattedDocumentField", false, "AlignRight", "e428af27-c4f7-4577-b80e-95a79f94322d"),
    ("FormattedDocumentField", false, "BackColor", "17724105-6e59-4d52-8a42-cf0fb4838037"),
    ("FormattedDocumentField", false, "Bold", "f20eefc2-f819-4ab1-be67-87b3ca2e26e6"),
    ("FormattedDocumentField", false, "BulletedList", "a0033f06-56f7-4855-b901-7ac66fe1bb99"),
    ("FormattedDocumentField", false, "CopyToClipboard", "7a294bdc-b86b-4b73-abc4-df9c811f61ef"),
    ("FormattedDocumentField", false, "CutToClipboard", "83670388-2e45-439e-9968-587eca6c7f8d"),
    ("FormattedDocumentField", false, "DecreaseFontSize", "ec647dcc-2be7-486c-9046-d8b371f9909e"),
    ("FormattedDocumentField", false, "DecreaseIndent", "39f6b9f1-7aa1-4a03-a01b-e127d51bc228"),
    ("FormattedDocumentField", false, "Font", "a8f6b59e-b712-4d3e-a974-55a3be4eb295"),
    ("FormattedDocumentField", false, "IncreaseFontSize", "0bdc43a3-79f0-48d6-bce4-1142542e1a59"),
    ("FormattedDocumentField", false, "IncreaseIndent", "d0a4d953-115b-4059-a6cb-6e67f903a4f3"),
    ("FormattedDocumentField", false, "Italic", "a8631f01-318a-4da2-80a9-9075c7524463"),
    ("FormattedDocumentField", false, "NumberedList", "2b5d0007-b74c-4786-a904-37e64bda8414"),
    ("FormattedDocumentField", false, "PasteFromClipboard", "905692d2-c3e7-4433-8f10-8d2ce35f652b"),
    ("FormattedDocumentField", false, "Picture", "9d8a3915-de52-4227-91cd-2fce22e09972"),
    ("FormattedDocumentField", false, "Preview", "4ca32834-6f9f-4dfb-89ce-6db36931c89b"),
    ("FormattedDocumentField", false, "Print", "a8483976-8b13-416a-9680-133b306dc6b0"),
    ("FormattedDocumentField", false, "SaveAs", "5a331cec-bf93-4af5-8f51-80fd7118db47"),
    ("FormattedDocumentField", false, "SelectAll", "b67f202a-dcf8-41f3-bda8-1ff9bed5f2ef"),
    ("FormattedDocumentField", false, "TextColor", "71007f7d-1995-44aa-9125-9926e70a35b5"),
    ("FormattedDocumentField", false, "Underline", "85bd789b-0047-46f9-9b2e-845907fc1b1d"),
    ("GraphicalSchemaField", false, "PageSetup", "01db2225-b62d-4112-a4b6-d39d627bf79f"),
    ("GraphicalSchemaField", false, "Preview", "1d13f9a3-402a-46cb-9c68-1709356840f2"),
    ("GraphicalSchemaField", false, "Print", "e2d6f793-b786-4640-a91b-8d77f73860f1"),
    ("PDFDocumentField", false, "GoBack", "32a87619-85ce-495a-a195-2719f5e9c71e"),
    ("PDFDocumentField", false, "GoForward", "85a1b7ee-e94d-4783-b519-4af123f58596"),
    ("PDFDocumentField", false, "GoToBegin", "a4e92b1d-5e86-4a86-b276-00e71ecf3fd4"),
    ("PDFDocumentField", false, "GoToEnd", "3e80231d-e169-456e-8373-0b9d4c15c8ab"),
    ("PDFDocumentField", false, "RotateClockwise", "30f0b852-3284-4ae4-838b-f89b78388bdc"),
    ("PDFDocumentField", false, "RotateCounterclockwise", "e28575de-9fe8-4a8e-a285-13250de33d49"),
    ("PDFDocumentField", false, "ScaleDown", "1e5ebd8b-32ee-4af9-b85c-3a0417000660"),
    ("PDFDocumentField", false, "ScaleUp", "d9117be6-f436-40fd-9670-8d71b99b2477"),
    ("SpreadSheetDocumentField", false, "AlignCenter", "ab0ebc39-68ee-4034-b2f4-43eee55bd651"),
    ("SpreadSheetDocumentField", false, "AlignJustify", "56ae90b6-588f-406e-919c-cc5cc7f86297"),
    ("SpreadSheetDocumentField", false, "AlignLeft", "87ecfbdd-8e2b-4ba2-a315-0897020f382f"),
    ("SpreadSheetDocumentField", false, "AlignRight", "e428af27-c4f7-4577-b80e-95a79f94322d"),
    ("SpreadSheetDocumentField", false, "BackColor", "17724105-6e59-4d52-8a42-cf0fb4838037"),
    ("SpreadSheetDocumentField", false, "Bold", "f20eefc2-f819-4ab1-be67-87b3ca2e26e6"),
    ("SpreadSheetDocumentField", false, "BorderAll", "4efdcc95-9f24-4652-a5ed-febfaa51f135"),
    ("SpreadSheetDocumentField", false, "BorderBottom", "5ff37d88-ba18-428c-bf49-9ccde4c46268"),
    ("SpreadSheetDocumentField", false, "BorderColor", "c1c95e8a-37b4-477c-9df1-7bc0fdbc1bd3"),
    ("SpreadSheetDocumentField", false, "BorderInside", "c4713141-07d1-411f-8804-172d4b8c4f01"),
    ("SpreadSheetDocumentField", false, "BorderLeft", "635136cc-3a47-43d1-8893-d7d536bf37c4"),
    ("SpreadSheetDocumentField", false, "BorderNone", "1177792e-7da2-4755-81a9-997f2d0e3dce"),
    ("SpreadSheetDocumentField", false, "BorderOutline", "05183b65-aa1f-401c-ac34-d99a6ff4ff60"),
    ("SpreadSheetDocumentField", false, "BorderRight", "e05dddd1-40ad-44a2-8ae7-0a551f0b1809"),
    ("SpreadSheetDocumentField", false, "BorderTop", "81be7ad1-a93f-4936-92d0-d59c10213f28"),
    ("SpreadSheetDocumentField", false, "ClearAll", "59e67a77-8141-42cf-b062-7cb92e210b6d"),
    ("SpreadSheetDocumentField", false, "ClearContent", "7eae9c22-db31-4f27-a56a-b4dd62d21a2c"),
    ("SpreadSheetDocumentField", false, "CollapseAllGroups", "ff5c34f8-b172-4ef2-91d3-48283a66a725"),
    ("SpreadSheetDocumentField", false, "CopyToClipboard", "1ba33890-92e9-42a3-95bd-a5c783f46d55"),
    ("SpreadSheetDocumentField", false, "DeleteColumns", "202efa3a-32e6-482c-8bc9-1a596bdd57f4"),
    ("SpreadSheetDocumentField", false, "DeleteComment", "531b9a9b-e4b3-4fe2-9562-1ff678c6d73d"),
    ("SpreadSheetDocumentField", false, "DeleteRows", "7b63d2c2-5b6b-472e-8f6c-7438d952be73"),
    ("SpreadSheetDocumentField", false, "Edit", "5c52ec51-6000-4190-b8c8-bd6201a271f5"),
    ("SpreadSheetDocumentField", false, "ExpandAllGroups", "12acffde-8389-4e5e-bd86-ff248262d84a"),
    ("SpreadSheetDocumentField", false, "Find", "0cf34151-92d3-42fd-954f-5938433908a4"),
    ("SpreadSheetDocumentField", false, "FindNext", "29fc1bfc-7bf8-4849-9b4b-3e42d12ebcbb"),
    ("SpreadSheetDocumentField", false, "FindPrevious", "474180f3-c5bd-49bf-bfd4-fddb03918295"),
    ("SpreadSheetDocumentField", false, "FixTable", "b34f83ab-1cd7-41b1-89bd-dd0804a47b26"),
    ("SpreadSheetDocumentField", false, "Font", "05e95a55-947e-4dde-a657-16ec11750a2f"),
    ("SpreadSheetDocumentField", false, "InsertComment", "ed6630f2-c296-43dd-b408-d370513fcebc"),
    ("SpreadSheetDocumentField", false, "InsertRows", "e5c3a5a6-695d-41bf-9c88-4367fd2a2a6e"),
    ("SpreadSheetDocumentField", false, "Italic", "a8631f01-318a-4da2-80a9-9075c7524463"),
    ("SpreadSheetDocumentField", false, "Merge", "f4df676f-eefb-48bd-9117-83ee6c207cb5"),
    ("SpreadSheetDocumentField", false, "PageSetup", "41c1bb40-1027-4fd1-a19e-17976100e64b"),
    ("SpreadSheetDocumentField", false, "PasteFromClipboard", "edf14e37-e755-4d1c-970c-48ed776e3a0e"),
    ("SpreadSheetDocumentField", false, "Preview", "5aa38159-2001-42ae-8451-f8cabe0762c3"),
    ("SpreadSheetDocumentField", false, "Print", "d673d512-f71a-48a6-ae5d-527a64ffd813"),
    ("SpreadSheetDocumentField", false, "PrintImmediately", "0e355e57-d603-4ac6-998b-c522c43d3668"),
    ("SpreadSheetDocumentField", false, "Properties", "be8800c3-8ccf-444a-bbf0-8f3078ff0ded"),
    ("SpreadSheetDocumentField", false, "RemoveName", "adc3d2d0-4d84-4038-a453-ab5d693a60bd"),
    ("SpreadSheetDocumentField", false, "Save", "d8e20c4d-3519-49aa-80e5-d6d66fee741a"),
    ("SpreadSheetDocumentField", false, "SaveAs", "999b786e-0534-4fca-8b62-f706d5336798"),
    ("SpreadSheetDocumentField", false, "SelectAll", "cca9b248-5f35-477f-a49d-95da3b7becad"),
    ("SpreadSheetDocumentField", false, "SetName", "5ce7de77-4796-41a0-beb3-7dc420053639"),
    ("SpreadSheetDocumentField", false, "ShowGrid", "83c3121e-60cc-4b47-a296-0c1976f0d766"),
    ("SpreadSheetDocumentField", false, "ShowGroups", "b55ad06a-ee91-4435-a747-6f51884772d9"),
    ("SpreadSheetDocumentField", false, "ShowHeaders", "4a9ec98e-c814-47b6-911f-694effc10fc5"),
    ("SpreadSheetDocumentField", false, "SplitCell", "02bdf755-7bc7-4e41-a199-f65cef10e6bc"),
    ("SpreadSheetDocumentField", false, "TextColor", "71007f7d-1995-44aa-9125-9926e70a35b5"),
    ("SpreadSheetDocumentField", false, "ThickBorderBottom", "65ee8b9b-8a0a-4de8-9a71-3a7a8c43c4ba"),
    ("SpreadSheetDocumentField", false, "ThickBorderOutline", "08872d94-c57a-470f-a050-0d4d81765df2"),
    ("SpreadSheetDocumentField", false, "ThickBorderTop", "a01654df-d7f1-4ec5-8b03-258d953de2e7"),
    ("SpreadSheetDocumentField", false, "Underline", "85bd789b-0047-46f9-9b2e-845907fc1b1d"),
    ("Table", false, "Add", "b0016a68-ec64-4e6d-b905-c71fd62efc4c"),
    ("Table", false, "AddFilterItem", "fca750bc-4fb6-40e2-ae0f-e818939a32e7"),
    ("Table", false, "AddAutoOrderItem", "48e12019-0fd6-46eb-aab6-2acba716a623"),
    ("Table", false, "AddFilterItemGroup", "a5fdef31-bbf0-4a9d-98aa-fd5fd8f1344a"),
    ("Table", false, "AddGroup", "7b70c79a-199e-4e87-a7eb-29dea9a5ad69"),
    ("Table", false, "AddOrderItem", "62ff963c-9426-43af-bb23-1d2ef3a9a0c1"),
    ("Table", false, "AddTable", "46647493-f1bf-4cd6-9110-6bfab80b62de"),
    ("Table", false, "CancelSearch", "44ad3ec9-f3c2-4913-9224-5f9fb6418743"),
    ("Table", false, "Change", "b41f5bbc-ba5d-4888-8cd1-db246a371418"),
    ("Table", false, "CheckAll", "18248aa8-e621-4e19-a611-54fb8923644c"),
    ("Table", false, "Choose", "8969c93a-23e5-4bef-941d-aaef315858d2"),
    ("Table", false, "ChooseAll", "15664824-eedc-4a92-9f6b-c89a2dead157"),
    ("Table", false, "Copy", "0ae4bea5-23be-42a7-b69e-97b11b29c453"),
    ("Table", false, "CopyToClipboard", "88078230-1f6b-415f-99e4-ad2ff73810cf"),
    ("Table", false, "Delete", "8d772f97-c0ef-47c0-9cb0-efea28c61341"),
    ("Table", false, "Detailed", "e6900951-1a42-4397-bf00-cabb2cd7ad6d"),
    ("Table", false, "EndEdit", "9ef79140-3de6-436a-8dda-610bb963f5db"),
    ("Table", false, "Find", "c0519548-2a9a-44de-a25e-faf01e089d4d"),
    ("Table", false, "FindByCurrentValue", "714d44cc-63da-4431-b33a-428e398d2a08"),
    ("Table", false, "GroupFilterItems", "4a817da0-5797-4e16-906f-02fb869e1873"),
    ("Table", false, "HierarchicalList", "01833a5a-6553-4c49-b445-095018107bb5"),
    ("Table", false, "List", "0d0249a4-2b2f-4fc0-a66f-b36f9494b3cc"),
    ("Table", false, "MoveDown", "fa51b106-eae6-44c7-8054-76cbb3100603"),
    ("Table", false, "MoveUp", "37740564-9e86-44a0-bea9-3f485a5a3f91"),
    ("Table", false, "OutputList", "49602716-fea6-497f-8047-726404038857"),
    ("Table", false, "Pickup", "59b4387d-f5be-4658-901f-bd3068217469"),
    ("Table", false, "SearchEverywhere", "7b683784-b474-441a-ba63-3d757bd0ffd4"),
    ("Table", false, "SearchHistory", "d96b0c03-b209-4d01-a3fc-17a14f873b64"),
    ("Table", false, "SelectAll", "51c99108-107c-43e1-8918-e48835bf2495"),
    ("Table", false, "SetPresentation", "7d4db5ed-0981-4020-b3b8-886b7165ba05"),
    ("Table", false, "ShowMultipleSelection", "e7216412-03ac-4a81-99c2-1d7c28e88e31"),
    ("Table", false, "ShowRowRearrangement", "8af6ebff-cd02-4bfe-a984-44a292623708"),
    ("Table", false, "SortListAsc", "2bbe4e12-06d2-409b-a972-eea585125d83"),
    ("Table", false, "SortListDesc", "58b2a785-23f6-4b0e-a324-9a1323285595"),
    ("Table", false, "Tree", "05468165-f954-45a5-84f2-6641c51f9f23"),
    ("Table", false, "UncheckAll", "5048cc44-702b-44e3-8445-9af75c02724d"),
    ("Table", false, "Ungroup", "82b88a24-2856-484a-afd9-55a15bdf9785"),
    ("Table", false, "UseFieldAsValue", "d7e55d2e-bfea-4d80-b4ad-a1bb31ec2147"),
    ("Table", false, "UserSettingItemProperties", "1f1e900a-8488-4159-81be-9704eb96906d"),
    // rt-uuids.md: corpus-pinned, and the platform's own registration for
    // names that only ever occur together.
    ("Table", true, "CreateByParameter", "b59f3c87-e213-4947-abae-9dbaffaef147"),
    ("Table", false, "Expand", "fc120c02-7f39-469b-b357-b2dd8d4b0765"),
    ("Table", false, "AddChart", "a10f1c0b-73ec-448f-b6d2-be0c86e95712"),
    ("Table", false, "AddNestedSchema", "e809ae75-11b6-480d-bc87-caf93b28236d"),
    ("Table", false, "UserSettings", "329bb47c-392f-4779-a1af-347d06bb624b"),
    ("Table", false, "Group", "33ff70c9-5df3-4907-9611-7649411f9180"),
    ("Table", false, "LoadSettings", "358196aa-1061-458a-8fba-e9cd11081205"),
    ("Table", false, "SaveSettings", "49a25ff2-06bc-4547-a119-a428f60bdfbf"),
    ("Table", false, "StandardSettings", "3bd8cc97-31ca-4fad-acf4-cc8f4d648a95"),
    ("SpreadSheetDocumentField", false, "ColumnWidth", "97407339-2c9f-400b-bd5b-3d97b6d00c21"),
    ("SpreadSheetDocumentField", false, "Ellipse", "93d90e38-02a4-42f8-828a-2798f51c4500"),
    ("SpreadSheetDocumentField", false, "GoToCell", "25d773e7-9961-49fc-a9c8-527079090143"),
    ("SpreadSheetDocumentField", false, "Group", "e406e2a0-f06b-4402-b8c3-9017c95df44c"),
    ("SpreadSheetDocumentField", false, "Hide", "b573b54a-ce87-4078-bd21-4f06709157c6"),
    ("SpreadSheetDocumentField", false, "InsertColumnsLeft", "468dca2b-17be-4657-bae8-64b94fcf6187"),
    ("SpreadSheetDocumentField", false, "InsertColumnsRight", "0a2d962b-5178-4fce-983b-19068b919f41"),
    ("SpreadSheetDocumentField", false, "InsertRowsBottom", "1d6dbce7-a813-437b-89b8-450319ed13bd"),
    ("SpreadSheetDocumentField", false, "InsertRowsTop", "4ecc8cf1-2a26-446f-9fb6-93db4ffee068"),
    ("SpreadSheetDocumentField", false, "Line", "c9b9e671-7c9b-44b5-97e9-dd1ee51a1bfd"),
    ("SpreadSheetDocumentField", false, "Picture", "a97ea34e-7af2-412c-aa9d-b3393b1914ac"),
    ("SpreadSheetDocumentField", false, "Rectangle", "852f0fba-4338-4c43-a2da-851fcffd07bb"),
    ("SpreadSheetDocumentField", false, "RowHeight", "17b9f6bb-74b3-439d-b719-eb236b2fe001"),
    ("SpreadSheetDocumentField", false, "SearchEverywhere", "ff533ae0-46a9-4e1d-aa3a-6dffa27e076b"),
    ("SpreadSheetDocumentField", false, "Show", "1b680da5-a5ca-4ea7-8db9-df079de39b61"),
    ("SpreadSheetDocumentField", false, "Text", "80455469-5f1c-4817-a992-756dfee9138f"),
    ("SpreadSheetDocumentField", false, "Ungroup", "88a56d46-abff-4925-91a2-6592a4664912"),
    ("SpreadSheetDocumentField", false, "AlignDrawingBottom", "9f71febd-8c22-4471-8410-31f455bb3c57"),
    ("SpreadSheetDocumentField", false, "AlignDrawingCenter", "f2b6b156-d929-4be2-af5b-9c9b792524bb"),
    ("SpreadSheetDocumentField", false, "AlignDrawingLeft", "ee0aab77-fd5f-4594-9c5e-e989a953642f"),
    ("SpreadSheetDocumentField", false, "AlignDrawingMiddle", "719daaab-c2d0-473d-b373-faf18ebe7d9d"),
    ("SpreadSheetDocumentField", false, "AlignDrawingRight", "80a0b41c-24df-40e4-8269-683fb557214d"),
    ("SpreadSheetDocumentField", false, "AlignDrawingTop", "f9395bfa-9301-4cec-8c1e-e2b62fb3abd6"),
    ("SpreadSheetDocumentField", false, "BringDrawingForward", "49a22a23-d2cf-4f84-97ae-66f94f863145"),
    ("SpreadSheetDocumentField", false, "BringDrawingToFront", "b383fa5a-2324-4e7e-a166-aabb5d64aea3"),
    ("SpreadSheetDocumentField", false, "CombineToGroup", "4402cb7a-f68e-44cd-9478-52a695b18a25"),
    ("SpreadSheetDocumentField", false, "DistributeDrawingsHorizontally", "60abcc40-dc62-4d03-833b-7b8ab8232d2c"),
    ("SpreadSheetDocumentField", false, "DistributeDrawingsVertically", "7f3f496d-506c-4239-98fb-58e1ea6ba54a"),
    ("SpreadSheetDocumentField", false, "EqualDrawingHeight", "f5773ab5-4036-49ca-8286-7a4ea2c354d7"),
    ("SpreadSheetDocumentField", false, "EqualDrawingSize", "fd523437-4160-4a52-a70b-9166c7eebcf0"),
    ("SpreadSheetDocumentField", false, "EqualDrawingWidth", "5ccf1fce-3fab-4fb6-ac04-a9b2cf689cee"),
    ("SpreadSheetDocumentField", false, "RemoveFromGroup", "69333d9f-28d1-446b-bd9a-cf8f85cf1704"),
    ("SpreadSheetDocumentField", false, "RemoveRepeatOnEachPage", "0e8c7cb4-f146-4208-af36-b3f8c7d71b66"),
    ("SpreadSheetDocumentField", false, "RepeatOnEachPage", "c50fd6b2-51a1-47e0-8cd3-84b16823287c"),
    ("SpreadSheetDocumentField", false, "SendDrawingBackward", "7e79f8d3-6cab-49d5-aac0-43f5056ed958"),
    ("SpreadSheetDocumentField", false, "SendDrawingToBack", "14bd1c58-da9d-41db-a515-75f8b39fdc52"),
    ("SpreadSheetDocumentField", false, "BlackAndWhiteView", "9e525e9b-99ed-4d89-9f02-2bf449ba65e6"),
    ("SpreadSheetDocumentField", false, "HeaderFooter", "2da58c85-ae4d-403f-b0e2-c50027a5467f"),
    ("SpreadSheetDocumentField", false, "InsertPageBreak", "952af05e-0771-4c26-adb6-a3418a262e4a"),
    ("SpreadSheetDocumentField", false, "Names", "feb51db7-bc1f-4b9f-a6e6-db24d5f812ab"),
    ("SpreadSheetDocumentField", false, "NextComment", "3e15759b-551a-46c4-8d24-8d6df22a1a64"),
    ("SpreadSheetDocumentField", false, "PageViewMode", "1c7e6bb5-54ac-4ebf-8823-e92b3cf629da"),
    ("SpreadSheetDocumentField", false, "PreviousComment", "e1ae173a-22c3-4909-a72c-5454b64c6446"),
    ("SpreadSheetDocumentField", false, "RemovePageBreak", "3a7ef674-f589-4734-9b22-954ea64dc79f"),
    ("SpreadSheetDocumentField", false, "RemovePrintArea", "41f3fbde-476a-4984-bd12-b32e990af811"),
    ("SpreadSheetDocumentField", false, "SetPrintArea", "6728e5c7-8f67-4b0d-bd6f-90b728218fe3"),
    ("SpreadSheetDocumentField", false, "ShowCellNames", "0c66c888-7512-402c-941d-96bec0e5749a"),
    ("SpreadSheetDocumentField", false, "ShowComments", "95dbc17e-d11e-4008-b9a9-24d5f5b1d061"),
    ("SpreadSheetDocumentField", false, "ShowRowAndColumnNames", "08fdfb5b-192a-41a9-b57a-9781cd3ef7b6"),
    ("SpreadSheetDocumentField", false, "Redo", "6f1ea963-0807-4de8-b544-b5666f500b05"),
    ("SpreadSheetDocumentField", false, "Undo", "f5814962-2bef-43dd-b633-a193d4b0970e"),
    ("FormattedDocumentField", false, "SearchEverywhere", "6e2f7ea0-a346-4c78-96d9-a0f512000910"),
    ("FormattedDocumentField", false, "Char", "871100d5-049d-4b22-a46a-fabf54bd64c3"),
    ("FormattedDocumentField", false, "Hyperlink", "6d83186a-5838-40a5-95e7-8990193adf0a"),
    ("FormattedDocumentField", false, "LineSpacing", "408f351e-0536-46be-8916-a891db9bfbe6"),
    ("FormattedDocumentField", false, "Redo", "6f1ea963-0807-4de8-b544-b5666f500b05"),
    ("FormattedDocumentField", false, "Strikeout", "db1cd9b3-bdf4-43f5-abd6-c2e4bd85d709"),
    ("FormattedDocumentField", false, "Undo", "f5814962-2bef-43dd-b633-a193d4b0970e"),
    ("Table", true, "AddFilterItem", "fca750bc-4fb6-40e2-ae0f-e818939a32e7"),
    ("Table", true, "CancelSearch", "44ad3ec9-f3c2-4913-9224-5f9fb6418743"),
    ("Table", true, "Change", "b41f5bbc-ba5d-4888-8cd1-db246a371418"),
    ("Table", true, "ChangeHistory", "11761e12-cf32-4826-a175-b23213e3b229"),
    ("Table", true, "Choose", "8969c93a-23e5-4bef-941d-aaef315858d2"),
    ("Table", true, "Copy", "0ae4bea5-23be-42a7-b69e-97b11b29c453"),
    ("Table", true, "CopyToClipboard", "88078230-1f6b-415f-99e4-ad2ff73810cf"),
    ("Table", true, "Create", "0f8d6d98-2f8b-405a-b8b3-0538e9d95da5"),
    ("Table", true, "CreateFolder", "d82ca05c-2966-4d77-9a39-a1eea087bfa7"),
    ("Table", true, "DynamicListStandardSettings", "33b7b9cd-6979-4435-8c58-d9bc8250edec"),
    ("Table", true, "Find", "c0519548-2a9a-44de-a25e-faf01e089d4d"),
    ("Table", true, "FindByCurrentValue", "714d44cc-63da-4431-b33a-428e398d2a08"),
    ("Table", true, "GetURL", "0e36114c-5b59-4005-9426-374a6c067e4a"),
    ("Table", true, "GroupFilterItems", "4a817da0-5797-4e16-906f-02fb869e1873"),
    ("Table", true, "HierarchicalList", "01833a5a-6553-4c49-b445-095018107bb5"),
    ("Table", true, "LevelDown", "dc118d99-b351-4e30-9310-e864f2e53ec0"),
    ("Table", true, "LevelUp", "0e9b637d-cf6e-4330-8a8f-cd44842e34bb"),
    ("Table", true, "List", "0d0249a4-2b2f-4fc0-a66f-b36f9494b3cc"),
    ("Table", true, "ListSettings", "14559f7c-853c-42a4-9ea1-01546107747b"),
    ("Table", true, "LoadDynamicListSettings", "182a793b-22a5-4625-b316-6a5be7f88078"),
    ("Table", true, "MoveDown", "fa51b106-eae6-44c7-8054-76cbb3100603"),
    ("Table", true, "MoveItem", "27bd521a-51c6-4fe7-846d-a98f988774b5"),
    ("Table", true, "MoveUp", "37740564-9e86-44a0-bea9-3f485a5a3f91"),
    ("Table", true, "OutputList", "825c1c15-ef8f-47ab-b002-e6b84b3e5b10"),
    ("Table", true, "Post", "e3dd8850-fc3c-41b1-bbb3-7c66af082608"),
    ("Table", true, "Refresh", "403bc6e6-b98e-4181-9f43-9c75cbbf82cf"),
    ("Table", true, "SaveDynamicListSettings", "95b4bc12-2ece-4d7a-b3e2-6f9293620a06"),
    ("Table", true, "SearchEverywhere", "7b683784-b474-441a-ba63-3d757bd0ffd4"),
    ("Table", true, "SearchHistory", "d96b0c03-b209-4d01-a3fc-17a14f873b64"),
    ("Table", true, "SetDateInterval", "daa306cd-a78a-4e74-a14c-739daba624cb"),
    ("Table", true, "SetDeletionMark", "a2f737a8-0114-4e86-a214-45e5c213fa65"),
    ("Table", true, "ShowMultipleSelection", "e7216412-03ac-4a81-99c2-1d7c28e88e31"),
    ("Table", true, "Tree", "05468165-f954-45a5-84f2-6641c51f9f23"),
    ("Table", true, "UndoPosting", "04ac7211-e74f-4776-9749-35a9282b1d52"),
];

/// The uuid a `<Button>` stores for `Form.StandardCommand.<name>`, by the
/// form's main attribute class and the command's name.
///
/// The table was first built from `<Button>` occurrences alone, so a name a
/// form only ever *excludes* was missing from it. The fifteen rows added for
/// those -- every one of the nine names the class-free table deliberately
/// omits, plus `ExecuteAndClose` -- were derived by reading each form's
/// stored command set (root trailer member `20 + 2 × <bag count>`) back
/// through the export's own `form_standard_command_suffix` and pairing it
/// with that form's own `<CommandSet>`: 4 249 of 4 276 ERP УХ forms and 257
/// of 261 BSP forms read back exactly, which forces the pairing. Of the 241
/// keys that gives, 124 were already here and all 124 agree -- 0
/// disagreements. The other 102 new keys are names the class-free table
/// already answers, so they are left out.
///
/// One key is not answered by this table: `(cfg:DynamicList, "Delete")` is
/// two uuids, and the list's `<MainTable>` decides. See
/// [`form_excluded_command_uuid`](crate::module_blob).
const FORM_STANDARD_COMMAND_UUIDS: &[(&str, &str, &str)] = &[
    ("", "Cancel", "679b62d9-ff72-4329-bf3a-c0c32b311dd2"),
    ("", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("", "Ignore", "d7e9e72c-8fa7-430c-a3e9-aeadfd57dfc7"),
    ("", "OpenFromStandaloneServer", "0ea1a92b-3477-44dd-b152-ea7d411f1c5d"),
    ("", "Retry", "5174ad3f-0569-42fd-8adf-011d8206db6c"),
    ("", "No", "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d"),
    ("", "OK", "f3613d5c-20c6-46e5-b4d5-7d712ece1296"),
    ("", "RestoreValues", "71e0226e-ebb2-4e33-8745-0a94a01bbf15"),
    ("", "SaveValues", "239f0103-8de9-4fdf-b485-eb5531da7e51"),
    ("", "Yes", "5d41082e-9619-42ec-b96f-98b082b3a2f0"),
    ("cfg:AccountingRegisterRecordSet", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:AccountingRegisterRecordSet", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:AccountingRegisterRecordSet", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("cfg:BusinessProcessObject", "Copy", "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988"),
    ("cfg:BusinessProcessObject", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:BusinessProcessObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:BusinessProcessObject", "OK", "f3613d5c-20c6-46e5-b4d5-7d712ece1296"),
    ("cfg:BusinessProcessObject", "Start", "8d7bcd38-1bbb-4dc1-a9ad-cc9d5966ca8e"),
    ("cfg:BusinessProcessObject", "StartAndClose", "e6a9041f-4d43-4f06-8e17-e95753531565"),
    ("cfg:BusinessProcessObject", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("cfg:CatalogObject", "Cancel", "679b62d9-ff72-4329-bf3a-c0c32b311dd2"),
    ("cfg:CatalogObject", "ChangeHistory", "174e58ce-82ad-4787-b956-9367937f7971"),
    ("cfg:CatalogObject", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:CatalogObject", "Copy", "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988"),
    ("cfg:CatalogObject", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:CatalogObject", "Delete", "c32d43de-b820-49d0-bf7a-d70829f48f40"),
    ("cfg:CatalogObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:CatalogObject", "Reread", "1f317795-c420-4a30-b594-c492abc55f7a"),
    ("cfg:CatalogObject", "RestoreValues", "71e0226e-ebb2-4e33-8745-0a94a01bbf15"),
    ("cfg:CatalogObject", "SaveValues", "239f0103-8de9-4fdf-b485-eb5531da7e51"),
    ("cfg:CatalogObject", "SetDeletionMark", "827b541d-30c1-4f06-aecf-92aa496a0835"),
    ("cfg:CatalogObject", "ShowInList", "3a17e914-ec6a-4280-b4df-78914f40522b"),
    ("cfg:CatalogObject", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("cfg:CatalogObject", "WriteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:ChartOfCalculationTypesObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:ChartOfCharacteristicTypesObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:ChartOfCharacteristicTypesObject", "SetDeletionMark", "827b541d-30c1-4f06-aecf-92aa496a0835"),
    ("cfg:ChartOfCharacteristicTypesObject", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("cfg:ChartOfCharacteristicTypesObject", "WriteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:ConstantsSet", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:ConstantsSet", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:ConstantsSet", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:ConstantsSet", "WriteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:DataProcessorObject", "Cancel", "679b62d9-ff72-4329-bf3a-c0c32b311dd2"),
    ("cfg:DataProcessorObject", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:DataProcessorObject", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:DataProcessorObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:DataProcessorObject", "Ignore", "d7e9e72c-8fa7-430c-a3e9-aeadfd57dfc7"),
    ("cfg:DataProcessorObject", "No", "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d"),
    ("cfg:DataProcessorObject", "OK", "f3613d5c-20c6-46e5-b4d5-7d712ece1296"),
    ("cfg:DataProcessorObject", "RestoreValues", "71e0226e-ebb2-4e33-8745-0a94a01bbf15"),
    ("cfg:DataProcessorObject", "SaveValues", "239f0103-8de9-4fdf-b485-eb5531da7e51"),
    ("cfg:DataProcessorObject", "Yes", "5d41082e-9619-42ec-b96f-98b082b3a2f0"),
    ("cfg:DocumentObject", "ChangeHistory", "174e58ce-82ad-4787-b956-9367937f7971"),
    ("cfg:DocumentObject", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:DocumentObject", "Copy", "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988"),
    ("cfg:DocumentObject", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:DocumentObject", "Delete", "c32d43de-b820-49d0-bf7a-d70829f48f40"),
    ("cfg:DocumentObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:DocumentObject", "Post", "3b8cedbc-8e74-4017-b901-d14b09f32f7a"),
    ("cfg:DocumentObject", "PostAndClose", "87317f86-057f-477e-9045-2da4e4980199"),
    ("cfg:DocumentObject", "Reread", "1f317795-c420-4a30-b594-c492abc55f7a"),
    ("cfg:DocumentObject", "RestoreValues", "71e0226e-ebb2-4e33-8745-0a94a01bbf15"),
    ("cfg:DocumentObject", "SetDeletionMark", "827b541d-30c1-4f06-aecf-92aa496a0835"),
    ("cfg:DocumentObject", "ShowInList", "3a17e914-ec6a-4280-b4df-78914f40522b"),
    ("cfg:DocumentObject", "UndoPosting", "389ef1f1-97ce-4326-adf5-886b2dead75c"),
    ("cfg:DocumentObject", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("cfg:DocumentObject", "WriteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:DynamicList", "Cancel", "679b62d9-ff72-4329-bf3a-c0c32b311dd2"),
    ("cfg:DynamicList", "CancelSearch", "96e0bc70-f8ff-4732-8119-060923203629"),
    ("cfg:DynamicList", "Change", "6886601d-276c-4d3f-af0a-05c586025608"),
    ("cfg:DynamicList", "ChangeHistory", "c9abb6b0-eafd-4505-8312-9a7b6888cbf3"),
    ("cfg:DynamicList", "Choose", "8e2b82cf-d1ea-46b2-afdf-a8d64e66ea2b"),
    ("cfg:DynamicList", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:DynamicList", "Copy", "342c531d-dc73-458a-8ac4-6a746916a33b"),
    ("cfg:DynamicList", "Create", "4f834c38-add1-45e4-a9f3-cefe3efac5c9"),
    ("cfg:DynamicList", "CreateFolder", "d8772fd1-a3bf-417d-8334-c49968dbb45e"),
    ("cfg:DynamicList", "CreateInitialImage", "62778a6d-6114-471c-93f7-e1ccd54bd266"),
    ("cfg:DynamicList", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:DynamicList", "DynamicListStandardSettings", "d603a249-6eb3-4e38-bb2d-a8a86a8ab156"),
    ("cfg:DynamicList", "Find", "bdefa701-6685-453e-a02a-3683d0cc16d3"),
    ("cfg:DynamicList", "FindByCurrentValue", "b520ca45-d8db-4982-b128-bb42a6afd911"),
    ("cfg:DynamicList", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:DynamicList", "HierarchicalList", "ffc5e8d5-40a7-4893-a590-49bd588f9466"),
    ("cfg:DynamicList", "LevelDown", "aa042316-63ba-4f10-8d39-3935474562d0"),
    ("cfg:DynamicList", "LevelUp", "e44f9b41-bf53-4837-b4d4-f0ff9cdf0feb"),
    ("cfg:DynamicList", "List", "a2b927a1-35af-43e3-af73-4af22ac2c0fa"),
    ("cfg:DynamicList", "ListSettings", "1c00edb8-a826-4855-9bde-94dbc5f620e5"),
    ("cfg:DynamicList", "LoadDynamicListSettings", "952c2984-9955-415a-8235-5c710aabe732"),
    ("cfg:DynamicList", "MoveItem", "39c6a2fb-45cc-41b1-853f-967fb68aa1df"),
    ("cfg:DynamicList", "No", "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d"),
    ("cfg:DynamicList", "OutputList", "9758d344-4b1d-4dc9-80bd-81060bc18b2a"),
    ("cfg:DynamicList", "Post", "4c569466-1af5-4fc1-9b63-7bf6493097bf"),
    ("cfg:DynamicList", "ReadChanges", "e7ae2a27-60a2-44ae-ab1d-f307d11c85bf"),
    ("cfg:DynamicList", "Refresh", "fd8f031f-c168-4e1b-8b0c-15eb3057e688"),
    ("cfg:DynamicList", "RestoreValues", "71e0226e-ebb2-4e33-8745-0a94a01bbf15"),
    ("cfg:DynamicList", "SaveDynamicListSettings", "d5c3842d-7252-4370-9174-756a6cc553e5"),
    ("cfg:DynamicList", "SaveValues", "239f0103-8de9-4fdf-b485-eb5531da7e51"),
    ("cfg:DynamicList", "SetDateInterval", "eb880cb2-a91f-4ad6-afb7-f0e6d7a1b111"),
    ("cfg:DynamicList", "SetDeletionMark", "2e86453d-8958-4c9a-a1b4-b15215eedc2e"),
    ("cfg:DynamicList", "ShowMultipleSelection", "9fea4ba9-7d33-47d4-a271-cb54df4a9b74"),
    ("cfg:DynamicList", "Tree", "0b83270d-7f95-4cdd-93c3-342d7991fed5"),
    ("cfg:DynamicList", "UndoPosting", "441362c1-0c86-4f73-bf50-6e1048a2db73"),
    ("cfg:DynamicList", "WriteChanges", "a29c4f3a-3b41-480a-a31e-5f9f73aa3216"),
    ("cfg:ExchangePlanObject", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:ExchangePlanObject", "Copy", "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988"),
    ("cfg:ExchangePlanObject", "CreateInitialImage", "d82e191e-f052-40ee-8691-00cac5b34629"),
    ("cfg:ExchangePlanObject", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:ExchangePlanObject", "Delete", "c32d43de-b820-49d0-bf7a-d70829f48f40"),
    ("cfg:ExchangePlanObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:ExchangePlanObject", "ReadChanges", "3328a951-c3c8-4f22-b99e-814f7cea6b82"),
    ("cfg:ExchangePlanObject", "SetDeletionMark", "827b541d-30c1-4f06-aecf-92aa496a0835"),
    ("cfg:ExchangePlanObject", "WriteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:ExchangePlanObject", "WriteChanges", "8b81add7-25af-4df7-a69c-144e3e3e4c8e"),
    ("cfg:InformationRegisterRecordManager", "ChangeHistory", "174e58ce-82ad-4787-b956-9367937f7971"),
    ("cfg:InformationRegisterRecordManager", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:InformationRegisterRecordManager", "Copy", "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988"),
    ("cfg:InformationRegisterRecordManager", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:InformationRegisterRecordManager", "Delete", "c32d43de-b820-49d0-bf7a-d70829f48f40"),
    ("cfg:InformationRegisterRecordManager", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:InformationRegisterRecordManager", "Reread", "1f317795-c420-4a30-b594-c492abc55f7a"),
    ("cfg:InformationRegisterRecordManager", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("cfg:InformationRegisterRecordManager", "WriteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:InformationRegisterRecordSet", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:InformationRegisterRecordSet", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:InformationRegisterRecordSet", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:InformationRegisterRecordSet", "Reread", "1f317795-c420-4a30-b594-c492abc55f7a"),
    ("cfg:InformationRegisterRecordSet", "RestoreValues", "71e0226e-ebb2-4e33-8745-0a94a01bbf15"),
    ("cfg:InformationRegisterRecordSet", "SaveValues", "239f0103-8de9-4fdf-b485-eb5531da7e51"),
    ("cfg:InformationRegisterRecordSet", "SwitchActivity", "f4613f71-5449-48ed-aea5-de005b272a1d"),
    ("cfg:InformationRegisterRecordSet", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("cfg:InformationRegisterRecordSet", "WriteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:ReportObject", "CancelEdit", "8149a06a-dbf3-4d4d-a275-5385a4196fc7"),
    ("cfg:ReportObject", "ChangeVariant", "fb9d7977-258a-440a-9b59-0a650c86f6a2"),
    ("cfg:ReportObject", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("cfg:ReportObject", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:ReportObject", "EndEdit", "74c1abd6-b274-4654-baf0-7b8418b792ea"),
    ("cfg:ReportObject", "Generate", "b5e6da6b-cec4-450c-876a-6a5f0837f6cc"),
    ("cfg:ReportObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:ReportObject", "LoadReportSettings", "b0c9afb6-320c-4e36-be21-8f6d48116415"),
    ("cfg:ReportObject", "LoadVariant", "b08b7a35-583a-4756-b814-0436ff9139c0"),
    ("cfg:ReportObject", "NewWindow", "03df6ee5-883c-4cc6-b319-d886d1a9b2c8"),
    ("cfg:ReportObject", "Print", "a11fe36e-0b45-4c07-80b3-2346b660a51e"),
    ("cfg:ReportObject", "ReportSettings", "0fb774df-ec1c-4e23-9ed1-e089974f74bf"),
    ("cfg:ReportObject", "RestoreValues", "71e0226e-ebb2-4e33-8745-0a94a01bbf15"),
    ("cfg:ReportObject", "Save", "a6d73055-3730-42e7-8934-3145ee987141"),
    ("cfg:ReportObject", "SaveReportSettings", "7910bb04-ddcc-4e5d-89f0-104c6ad0f187"),
    ("cfg:ReportObject", "SaveValues", "239f0103-8de9-4fdf-b485-eb5531da7e51"),
    ("cfg:ReportObject", "SaveVariant", "9bffcf73-7b1d-4a8d-bf23-5e051af3ee29"),
    ("cfg:TaskObject", "Copy", "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988"),
    ("cfg:TaskObject", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("cfg:TaskObject", "Delete", "c32d43de-b820-49d0-bf7a-d70829f48f40"),
    // The task-object spelling of `WriteAndClose`'s uuid. 17 ERP УХ and 3 BSP
    // forms exclude it, and no form ever excludes both spellings.
    ("cfg:TaskObject", "ExecuteAndClose", "32df4349-2607-4c2b-a4b9-bca4a1a28bd7"),
    ("cfg:TaskObject", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
    ("cfg:TaskObject", "Reread", "1f317795-c420-4a30-b594-c492abc55f7a"),
    ("cfg:TaskObject", "SetDeletionMark", "827b541d-30c1-4f06-aecf-92aa496a0835"),
    ("cfg:TaskObject", "Write", "fe558fde-99b3-45d0-a060-9fc2905309f6"),
    ("dcsset:SettingsComposer", "CancelEdit", "8149a06a-dbf3-4d4d-a275-5385a4196fc7"),
    ("xs:string", "Cancel", "679b62d9-ff72-4329-bf3a-c0c32b311dd2"),
    ("xs:string", "Close", "3772996b-41f4-4c47-a5a8-ea397db424ae"),
    ("xs:string", "CustomizeForm", "198ea630-fda2-4cda-8a23-f999f4c67ee6"),
    ("xs:string", "Help", "39bb0fe9-771d-4dd5-8a6e-2d16984523af"),
];

/// The uuid of `Form.Item.<item>.StandardCommand.<name>`, or `None` when the
/// corpus never stored one for that target and name.
/// `item_standard_command_uuid`, with the two names a plain table's own
/// binding splits (rt-uuids.md §1): `Ungroup` on a settings-structure table
/// (`….Settings`) is `23802256-…` and on a filter `82b88a24-…`; `Choose` on a
/// table of available fields is `d77e5787-…` and elsewhere `8969c93a-…`.
pub(crate) fn table_item_standard_command_uuid(
    tag: &str,
    dynamic_list: bool,
    data_path: Option<&str>,
    name: &str,
) -> Option<&'static str> {
    if tag == "Table" && !dynamic_list {
        let path = data_path.unwrap_or("").trim();
        let last = path.rsplit('.').next().unwrap_or("");
        match name {
            "Ungroup" if path.ends_with(".Settings") => {
                return Some("23802256-7145-47c7-b379-8d60ca1b1262");
            }
            "Choose" if last.ends_with("AvailableFields") => {
                return Some("d77e5787-b130-4355-8f8f-01ecec82f843");
            }
            _ => {}
        }
    }
    item_standard_command_uuid(tag, dynamic_list, name)
}

pub(crate) fn item_standard_command_uuid(
    tag: &str,
    dynamic_list: bool,
    name: &str,
) -> Option<&'static str> {
    ITEM_STANDARD_COMMAND_UUIDS
        .iter()
        .find_map(|(candidate, dynamic, command, uuid)| {
            (*candidate == tag && *dynamic == dynamic_list && *command == name).then_some(*uuid)
        })
}

/// `Delete` on a dynamic list, which two uuids answer.
///
/// The list's `<Settings><MainTable>` decides, and it is a clean partition
/// over all 608 forms of both corpora that exclude it: a table of kind
/// `InformationRegister` stores `1cc781aa-…` in 62 of 62, and the seven other
/// kinds seen -- `Document`, `Catalog`, `DocumentJournal`, `BusinessProcess`,
/// `ChartOfCharacteristicTypes`, `ExchangePlan` and no table at all -- store
/// `3dd3bd8a-…` in 546 of 546. A register has no deletion mark, so its list's
/// `Delete` is the direct-delete command.
///
/// A kind outside those seven is unmeasured and gets no answer.
pub(crate) fn dynamic_list_delete_command_uuid(main_table_kind: Option<&str>) -> Option<&'static str> {
    match main_table_kind {
        Some("InformationRegister") => Some("1cc781aa-f32b-4dc7-996a-6c38c3deda5c"),
        None
        | Some(
            "Document"
            | "Catalog"
            | "DocumentJournal"
            | "BusinessProcess"
            | "ChartOfCharacteristicTypes"
            | "ExchangePlan",
        ) => Some("3dd3bd8a-ac1e-44d6-ac83-e7802642a5e2"),
        Some(_) => None,
    }
}

/// The uuid of an item's own `Delete` -- a `<Table>`'s `<ExcludedCommand>`
/// or a button's `Form.Item.<table>.StandardCommand.Delete` -- when the table
/// shows a dynamic list. Measured over 445 tables and 59 buttons of both
/// corpora: a list over an `InformationRegister` stores the plain table's
/// `8d772f97-…` in 101 of 101 tables, and a list over a kind with a deletion
/// mark stores `ec576e13-…` in 344 of 344.
///
/// A kind outside those measured gets no answer.
pub(crate) fn dynamic_list_item_delete_command_uuid(
    main_table_kind: Option<&str>,
) -> Option<&'static str> {
    match main_table_kind {
        Some("InformationRegister") => Some("8d772f97-c0ef-47c0-9cb0-efea28c61341"),
        Some(
            "Document"
            | "Catalog"
            | "DocumentJournal"
            | "BusinessProcess"
            | "ChartOfCharacteristicTypes"
            | "ExchangePlan",
        ) => Some("ec576e13-1e76-4c33-98aa-a33204514227"),
        _ => None,
    }
}

/// Whether any form's class stores a uuid for a standard command of this
/// name, which is what tells a spelling the corpus knows from one it does not.
pub(crate) fn is_form_standard_command(name: &str) -> bool {
    FORM_STANDARD_COMMAND_UUIDS
        .iter()
        .any(|(_, command, _)| *command == name)
}

/// The uuid of `Form.StandardCommand.<name>` for a form whose main attribute
/// is of this class.
///
/// A pair the corpus never showed falls back to the name alone when every
/// class that does show the name stores one uuid for it -- `Write` is
/// `fe558fde-…` on all nine classes that have it, and a `ConstantsSet` form's
/// button stores the same. A name whose classes disagree gets no answer.
pub(crate) fn form_standard_command_uuid(
    main_attribute_class: &str,
    name: &str,
) -> Option<&'static str> {
    if let Some(uuid) = FORM_STANDARD_COMMAND_UUIDS
        .iter()
        .find_map(|(candidate, command, uuid)| {
            (*candidate == main_attribute_class && *command == name).then_some(*uuid)
        })
    {
        return Some(uuid);
    }
    let mut uuids = FORM_STANDARD_COMMAND_UUIDS
        .iter()
        .filter(|(_, command, _)| *command == name)
        .map(|(_, _, uuid)| *uuid);
    let first = uuids.next()?;
    uuids.all(|uuid| uuid == first).then_some(first)
}

/// The `{31,…}` record of a `<Button>` whose action is a form standard
/// command and which carries nothing but its name and its tooltip.
///
/// One shape across every such button of the bodies read for this module: only
/// the id, the name, the command uuid and the tooltip differ.
pub(crate) fn format_standard_command_button(
    id: &str,
    name: &str,
    command_uuid: &str,
    extended_tooltip_id: &str,
    extended_tooltip_name: &str,
) -> String {
    format!(
        "{{31,{{{id},{ns}}},0,1,{{0,{{0,{{\"B\",1}},0}}}},0,{name},{{1,0}},1,{{1,{command_uuid}}},\
         {{0}},3,0,0,0,2,2,0,0,0,{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},\
         {{0,0,0}},0,{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},1,{{\"Pattern\"}},\"\",2,0,1,{tooltip},\
         {{\"U\"}},1,0,0,1,0,0,0,3,3,3,0,0,0,0,0,0,1,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
        tooltip = format_extended_tooltip(extended_tooltip_id, extended_tooltip_name),
    )
}

/// The `{12,…}` record of an item's `<ExtendedTooltip>` in the shape the
/// platform writes when the owning item carries the visibility tuple.
///
/// The same record as [`format_extended_tooltip`] with `1,{0,{0,{"B",1},0}}`
/// inserted after the id -- the common `UserVisible` prefix the reader already
/// normalises away. 49 of the 1 602 tooltip records read for this module carry
/// it; the other 1 475 do not.
pub(crate) fn format_visible_extended_tooltip(id: &str, name: &str) -> String {
    format!(
        "{{12,{{{id},{ns}}},0,0,1,{{0,{{0,{{\"B\",1}},0}}}},0,{name},{{1,0}},{{1,0}},1,0,0,2,2,\
         {{3,4,{{0}}}},{{7,3,0,1,100}},{{0,0,0}},1,{{5,0,0,3,0,{{0,1,0}},{{3,4,{{0}}}},\
         {{3,4,{{0}}}},{{3,0,{{0}},0,1,0,{appearance}}}}},0,1,2,{{1,{{1,0}},0}},0,0,1,0,0,1,0,3,3,0,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
        appearance = DEFAULT_APPEARANCE_UUID,
    )
}

/// A `<Title>` that declares one Russian line, in the shape a body stores it.
/// An empty title is `{1,0}`.
pub(crate) fn format_russian_title(text: &str) -> String {
    if text.is_empty() {
        return "{1,0}".to_string();
    }
    format!("{{1,1,{{\"ru\",{}}}}}", quoted(text))
}

/// What a `<LabelDecoration>` record needs beyond its own name.
pub(crate) struct NativeLabelDecoration<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    /// Already formatted -- see [`format_russian_title`].
    pub(crate) title: &'a str,
    /// The picture the decoration shows, when it shows one.
    pub(crate) picture_uuid: Option<&'a str>,
    pub(crate) context_menu_id: &'a str,
    pub(crate) context_menu_name: &'a str,
    pub(crate) extended_tooltip_id: &'a str,
    pub(crate) extended_tooltip_name: &'a str,
}

/// The `{12,…}` record of a `<LabelDecoration>` that carries its name, its
/// title and its two default children.
///
/// Measured against `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов`,
/// whose decorations are exactly this shape.
pub(crate) fn format_label_decoration(decoration: &NativeLabelDecoration<'_>) -> String {
    let picture = match decoration.picture_uuid {
        Some(uuid) => format!("{{4,1,{{0,{uuid}}},\"\",-1,-1,0,0,\"\"}}"),
        None => "{4,0,{0},\"\",-1,-1,1,0,\"\"}".to_string(),
    };
    format!(
        "{{12,{{{id},{ns}}},0,0,0,1,{name},{title},{{1,0}},1,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{4,{picture},0,0,0,{{1,0}},{{3,4,{{0}}}},\
         {{3,0,{{0}},0,1,0,{appearance}}},0,0,{{0,1,0}},0,100}},1,{context_menu},1,2,\
         {{1,{title},0}},0,1,{tooltip},1,0,0,1,0,3,3,0,0}}",
        id = decoration.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(decoration.name),
        title = decoration.title,
        appearance = DEFAULT_APPEARANCE_UUID,
        // This formatter builds a decoration whose menu the caller has
        // already refused children for, so the empty menu is right here.
        context_menu = format_field_context_menu(
            decoration.context_menu_id,
            decoration.context_menu_name,
            None,
            &[],
        ),
        tooltip = format_extended_tooltip(
            decoration.extended_tooltip_id,
            decoration.extended_tooltip_name
        ),
    )
}

/// The namespace a form *command* id lives in, which is not the item one.
pub(crate) const FORM_COMMAND_NAMESPACE_UUID: &str = "409b9a53-7f7e-4178-86c1-33176c7c7a7a";

/// What a table says about itself before its columns -- the 54 members that
/// open a `{55,…}` record.
///
/// The head is followed by a **keyed property bag exactly like the root's** --
/// a count, then that many `(key, value)` pairs -- then the table's events,
/// `{0}`, and its two children, the context menu and the command bar, each
/// behind a flag. So a table head is `59 + 2 x <bag size>` members long once
/// its children are lifted out, which is **every length the corpus stores**:
/// 59, 61, 63, 77, 79, 81, 83 and 85, with nothing else.
///
/// Measured over all 6 903 table records of ERP УХ: **6 900 of the 54 fixed
/// members rebuild byte for byte (99.96%)**. The three that differ carry a
/// `<Shortcut>` at member 51, which the caller supplies.
pub(crate) struct NativeTableHead<'a> {
    pub(crate) id: &'a str,
    /// `<Representation>`: `List` 0, `Tree` 2, and 1 when the table says
    /// nothing.
    pub(crate) representation: Option<&'a str>,
    /// The functional-options block, when the table restricts itself.
    pub(crate) functional_options: Option<&'a str>,
    pub(crate) name: &'a str,
    /// `<TitleLocation>` and `<TitleHeight>`.
    pub(crate) title_location: Option<&'a str>,
    pub(crate) title_height: Option<&'a str>,
    /// `<CommandBarLocation>`: `None` 0, `Auto` 1, `Top` 2, `Bottom` 3.
    pub(crate) command_bar_location: Option<&'a str>,
    /// Already formatted -- the title, the tooltip title and the data path.
    pub(crate) title: &'a str,
    pub(crate) tooltip_title: &'a str,
    pub(crate) data_path: &'a str,
    pub(crate) autofill: bool,
    pub(crate) enabled: bool,
    pub(crate) read_only: bool,
    pub(crate) default_item: bool,
    /// `<ChangeRowSet>` and `<ChangeRowOrder>`, on unless turned off.
    pub(crate) change_row_set: bool,
    pub(crate) change_row_order: bool,
    pub(crate) width: Option<&'a str>,
    pub(crate) height: Option<&'a str>,
    /// `<HeightInTableRows>`.
    pub(crate) height_in_table_rows: Option<&'a str>,
    pub(crate) choice_mode: bool,
    /// `<RowInputMode>`: `EndOfWindow` 1, `AfterCurrentRow` 2.
    pub(crate) row_input_mode: Option<&'a str>,
    /// `<SelectionMode>`: `SingleRow` 0, anything else 1.
    pub(crate) selection_mode: Option<&'a str>,
    /// `<RowSelectionMode>`: `Row` 1.
    pub(crate) row_selection_mode: Option<&'a str>,
    /// `<Header>` and `<Footer>`, with their heights.
    pub(crate) header: bool,
    pub(crate) header_height: Option<&'a str>,
    pub(crate) footer: bool,
    pub(crate) footer_height: Option<&'a str>,
    /// `<HorizontalScrollBar>` and `<VerticalScrollBar>`: `DontUse` 0,
    /// `UseAlways` 1, and 2 when the table says nothing.
    pub(crate) horizontal_scroll_bar: Option<&'a str>,
    pub(crate) vertical_scroll_bar: Option<&'a str>,
    /// `<HorizontalLines>` and `<VerticalLines>`, on unless turned off.
    pub(crate) horizontal_lines: bool,
    pub(crate) vertical_lines: bool,
    pub(crate) use_alternation_row_color: bool,
    pub(crate) auto_insert_new_row: bool,
    /// `<InitialListView>`: `Beginning` 0, `End` 1, and 2 when unnamed.
    pub(crate) initial_list_view: Option<&'a str>,
    /// `<InitialTreeView>`: `ExpandTopLevel` 1, `ExpandAllLevels` 2.
    pub(crate) initial_tree_view: Option<&'a str>,
    /// `<Output>`: `Enable` 1, `Disable` 2.
    pub(crate) output: Option<&'a str>,
    pub(crate) horizontal_stretch: bool,
    pub(crate) vertical_stretch: bool,
    /// The row picture's data path, the picture, the five appearance blocks
    /// and the two fonts, already formatted.
    pub(crate) row_picture_data_path: &'a str,
    pub(crate) row_picture: &'a str,
    pub(crate) appearance: [&'a str; 6],
    /// Member 51: the `<Shortcut>` block, `{0,0,0}` when the table names none.
    pub(crate) shortcut: &'a str,
    pub(crate) enable_start_drag: bool,
    pub(crate) enable_drag: bool,
}

impl Default for NativeTableHead<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            representation: None,
            functional_options: None,
            name: "",
            title_location: None,
            title_height: None,
            command_bar_location: None,
            title: "{1,0}",
            tooltip_title: "{1,0}",
            data_path: "{0}",
            autofill: false,
            enabled: true,
            read_only: false,
            default_item: false,
            change_row_set: true,
            change_row_order: true,
            width: None,
            height: None,
            height_in_table_rows: None,
            choice_mode: false,
            row_input_mode: None,
            selection_mode: None,
            row_selection_mode: None,
            header: true,
            header_height: None,
            footer: false,
            footer_height: None,
            horizontal_scroll_bar: None,
            vertical_scroll_bar: None,
            horizontal_lines: true,
            vertical_lines: true,
            use_alternation_row_color: false,
            auto_insert_new_row: false,
            initial_list_view: None,
            initial_tree_view: None,
            output: None,
            horizontal_stretch: true,
            vertical_stretch: true,
            row_picture_data_path: "{0}",
            row_picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            appearance: [
                "{3,4,{0}}",
                "{3,4,{0}}",
                "{3,4,{0}}",
                "{7,3,0,1,100}",
                "{3,4,{0}}",
                "{7,3,0,1,100}",
            ],
            shortcut: "{0,0,0}",
            enable_start_drag: false,
            enable_drag: false,
        }
    }
}

/// The 54 members that open a `{55,…}` table record.
pub(crate) fn format_table_head(head: &NativeTableHead<'_>) -> Option<String> {
    let options = match head.functional_options {
        Some(block) => format!("1,{block}"),
        None => "0".to_string(),
    };
    let representation = root_code(
        head.representation,
        &[("List", "0"), ("Tree", "2")],
        "1",
    )?;
    let title_location = root_code(
        head.title_location,
        &[
            ("Auto", "1"),
            ("Left", "2"),
            ("Top", "3"),
            ("Right", "4"),
            ("Bottom", "5"),
        ],
        "0",
    )?;
    let command_bar_location = root_code(
        head.command_bar_location,
        &[("None", "0"), ("Auto", "1"), ("Top", "2"), ("Bottom", "3")],
        "1",
    )?;
    let row_input_mode = root_code(
        head.row_input_mode,
        &[("EndOfWindow", "1"), ("AfterCurrentRow", "2")],
        "0",
    )?;
    let selection_mode = root_code(
        head.selection_mode,
        &[("SingleRow", "0"), ("MultiRow", "1")],
        "1",
    )?;
    let row_selection_mode = root_code(head.row_selection_mode, &[("Row", "1"), ("Cell", "0")], "0")?;
    let horizontal_scroll = root_code(
        head.horizontal_scroll_bar,
        &[("DontUse", "0"), ("UseAlways", "1")],
        "2",
    )?;
    let vertical_scroll = root_code(
        head.vertical_scroll_bar,
        &[("DontUse", "0"), ("UseAlways", "1")],
        "2",
    )?;
    let initial_list = root_code(
        head.initial_list_view,
        &[("Beginning", "0"), ("End", "1")],
        "2",
    )?;
    let initial_tree = root_code(
        head.initial_tree_view,
        &[("ExpandTopLevel", "1"), ("ExpandAllLevels", "2")],
        "0",
    )?;
    let output = root_code(head.output, &[("Enable", "1"), ("Disable", "2")], "0")?;
    Some(format!(
        "55,{{{id},{ns}}},0,{representation},{options},{name},{title_location},{title_height},\
         {command_bar_location},{title},{tooltip_title},{data_path},{autofill},{enabled},\
         {read_only},0,{default_item},{change_row_set},{change_row_order},{width},{height},\
         {height_in_table_rows},{choice_mode},{row_input_mode},{selection_mode},\
         {row_selection_mode},{header},{header_height},{footer},{footer_height},\
         {horizontal_scroll},{vertical_scroll},{horizontal_lines},{vertical_lines},0,0,\
         {alternation},{auto_insert},{initial_list},{initial_tree},{output},\
         {horizontal_stretch},{vertical_stretch},{row_picture_data_path},{row_picture},\
         {a0},{a1},{a2},{a3},{a4},{a5},{shortcut},{enable_start_drag},{enable_drag}",
        id = head.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(head.name),
        title_height = head.title_height.unwrap_or("0"),
        title = head.title,
        tooltip_title = head.tooltip_title,
        data_path = head.data_path,
        autofill = u8::from(head.autofill),
        enabled = u8::from(head.enabled),
        read_only = u8::from(head.read_only),
        default_item = u8::from(head.default_item),
        change_row_set = u8::from(head.change_row_set),
        change_row_order = u8::from(head.change_row_order),
        width = head.width.unwrap_or("0"),
        height = head.height.unwrap_or("0"),
        height_in_table_rows = head.height_in_table_rows.unwrap_or("0"),
        choice_mode = u8::from(head.choice_mode),
        header = u8::from(head.header),
        header_height = head.header_height.unwrap_or("1"),
        footer = u8::from(head.footer),
        footer_height = head.footer_height.unwrap_or("1"),
        horizontal_lines = u8::from(head.horizontal_lines),
        vertical_lines = u8::from(head.vertical_lines),
        alternation = u8::from(head.use_alternation_row_color),
        auto_insert = u8::from(head.auto_insert_new_row),
        horizontal_stretch = u8::from(head.horizontal_stretch),
        vertical_stretch = u8::from(head.vertical_stretch),
        row_picture_data_path = head.row_picture_data_path,
        row_picture = head.row_picture,
        a0 = head.appearance[0],
        a1 = head.appearance[1],
        a2 = head.appearance[2],
        a3 = head.appearance[3],
        a4 = head.appearance[4],
        a5 = head.appearance[5],
        shortcut = head.shortcut,
        enable_start_drag = u8::from(head.enable_start_drag),
        enable_drag = u8::from(head.enable_drag),
    ))
}

/// The `{55,…}` record of a table, whole.
pub(crate) fn format_table_record(
    head: &str,
    properties: &[(&str, String)],
    events: &str,
    command_set: &str,
    context_menu: &str,
    command_bar: &str,
    columns: &[(&str, String)],
    tail: &str,
) -> String {
    let mut bag = String::new();
    for (key, value) in properties {
        bag.push(',');
        bag.push_str(key);
        bag.push(',');
        bag.push_str(value);
    }
    let mut body = String::new();
    for (kind_uuid, record) in columns {
        body.push(',');
        body.push_str(kind_uuid);
        body.push(',');
        body.push_str(record);
    }
    format!(
        "{{{head},{count}{bag},{events},{command_set},1,{context_menu},1,{command_bar},\
         {columns}{body},{tail}}}",
        count = properties.len(),
        columns = columns.len(),
    )
}

/// What a table says about itself after its columns -- the 37 members that
/// close a `{55,…}` record.
///
/// A table's columns are `(kind uuid, record)` pairs like a group's children,
/// and what follows them is **exactly 37 members in all 6 903 table records**
/// of ERP УХ that could be split. **6 890 of them rebuild byte for byte**;
/// the 13 left differ in two members whose driver is not in the element.
///
/// The one reading that could not be guessed is member 35: it is
/// `<FileDragMode>`, and it is **1 when the table does not name one** and 0
/// when it names `AsFile`. Every one of the 4 593 tables that name it writes
/// 0 and every one of the 2 310 that do not writes 1, with nothing in between.
/// Reading it as a drag flag, which three witnesses suggested, made the
/// measurement worse rather than better.
pub(crate) struct NativeTableTail<'a> {
    /// `<AutoMarkIncomplete>` and `<AutoAddIncomplete>`: 2 when unnamed.
    pub(crate) auto_mark_incomplete: Option<bool>,
    pub(crate) auto_add_incomplete: Option<bool>,
    pub(crate) visible: bool,
    pub(crate) multiple_choice: bool,
    /// `<SkipOnInput>`: 2 when unnamed.
    pub(crate) skip_on_input: Option<bool>,
    /// `<SearchOnInput>`: `Use` or `DontUse`.
    pub(crate) search_on_input: Option<&'a str>,
    /// `<ToolTipRepresentation>`.
    pub(crate) tooltip_representation: Option<&'a str>,
    /// The table's extended tooltip, already formatted.
    pub(crate) extended_tooltip: &'a str,
    /// Where the table shows its search string, its view status and its search
    /// control.
    pub(crate) search_string_location: Option<&'a str>,
    pub(crate) view_status_location: Option<&'a str>,
    pub(crate) search_control_location: Option<&'a str>,
    /// The three `{5,…}` additions, always present, already formatted.
    pub(crate) additions: [&'a str; 3],
    /// `<RefreshRequest>`, of which only `PullFromTop` is ever stored.
    pub(crate) refresh_request: Option<&'a str>,
    pub(crate) auto_max_width: bool,
    pub(crate) max_width: Option<&'a str>,
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: Option<&'a str>,
    /// `<HeightControlVariant>`: `UseHeightInFormRows`, `UseHeightInTableRows`
    /// or `UseContentHeight`.
    pub(crate) height_control_variant: Option<&'a str>,
    pub(crate) auto_max_rows_count: bool,
    pub(crate) max_rows_count: Option<&'a str>,
    /// `<CurrentRowUse>`: `Choice`, `SelectionPresentation` or
    /// `SelectionPresentationAndChoice`.
    pub(crate) current_row_use: Option<&'a str>,
    /// The member after it, `<BehaviorOnHorizontalCompression>`: absent 0,
    /// `MoveItemsByImportance` 2 (the exporter's reverse offset 4).
    pub(crate) behavior_on_horizontal_compression: Option<&'a str>,
    /// `<FileDragMode>`, of which only `AsFile` is ever stored.
    pub(crate) file_drag_mode: Option<&'a str>,
    /// Tail members 27, 28 and 34: `<GroupHorizontalAlign>`,
    /// `<GroupVerticalAlign>` and the `DisplayImportance` attribute.
    pub(crate) group_horizontal_align: Option<&'a str>,
    pub(crate) group_vertical_align: Option<&'a str>,
    pub(crate) display_importance: &'a str,
}

impl Default for NativeTableTail<'_> {
    fn default() -> Self {
        Self {
            auto_mark_incomplete: None,
            auto_add_incomplete: None,
            visible: true,
            multiple_choice: false,
            skip_on_input: None,
            search_on_input: None,
            tooltip_representation: None,
            extended_tooltip: "",
            search_string_location: None,
            view_status_location: None,
            search_control_location: None,
            additions: ["", "", ""],
            refresh_request: None,
            auto_max_width: true,
            max_width: None,
            auto_max_height: true,
            max_height: None,
            height_control_variant: None,
            auto_max_rows_count: true,
            max_rows_count: None,
            current_row_use: None,
            behavior_on_horizontal_compression: None,
            file_drag_mode: None,
            group_horizontal_align: None,
            group_vertical_align: None,
            display_importance: "0",
        }
    }
}

/// The 37 members that close a `{55,…}` table record.
pub(crate) fn format_table_tail(tail: &NativeTableTail<'_>) -> Option<String> {
    let tooltip_representation = root_code(
        tail.tooltip_representation,
        &[
            ("Auto", "0"),
            ("None", "1"),
            ("Balloon", "2"),
            ("Button", "3"),
            ("ShowAuto", "4"),
            ("ShowTop", "5"),
            ("ShowLeft", "6"),
            ("ShowBottom", "7"),
            ("ShowRight", "8"),
        ],
        "0",
    )?;
    let search_string = root_code(
        tail.search_string_location,
        &[
            ("None", "1"),
            ("CommandBar", "2"),
            ("Top", "3"),
            ("Bottom", "4"),
            ("FormCaption", "5"),
            ("PullFromTop", "6"),
        ],
        "0",
    )?;
    let view_status = root_code(
        tail.view_status_location,
        &[("None", "1"), ("Top", "2"), ("Bottom", "3")],
        "0",
    )?;
    let search_control = root_code(
        tail.search_control_location,
        &[("None", "1"), ("CommandBar", "2")],
        "0",
    )?;
    let refresh = root_code(tail.refresh_request, &[("PullFromTop", "1")], "0")?;
    let height_variant = root_code(
        tail.height_control_variant,
        &[
            ("UseHeightInFormRows", "1"),
            ("UseHeightInTableRows", "2"),
            ("UseContentHeight", "3"),
        ],
        "0",
    )?;
    let current_row_use = root_code(
        tail.current_row_use,
        &[
            ("Choice", "1"),
            ("SelectionPresentation", "2"),
            ("SelectionPresentationAndChoice", "3"),
        ],
        "0",
    )?;
    let search_on_input = root_code(
        tail.search_on_input,
        &[("Use", "0"), ("DontUse", "1")],
        "2",
    )?;
    // A table that does not name a drag mode writes 1, not 0.
    let drag = root_code(tail.file_drag_mode, &[("AsFile", "0")], "1")?;
    Some(format!(
        "{auto_mark},{auto_add},{visible},{multiple_choice},{{\"Pattern\"}},\"\",\"\",\
         {skip_on_input},{search_on_input},{tooltip_representation},1,{extended_tooltip},\
         {search_string},{view_status},{search_control},1,{first},1,{second},1,{third},\
         {refresh},{auto_max_width},{max_width},0,{auto_max_height},{max_height},\
         {group_horizontal},{group_vertical},{height_variant},{auto_max_rows},{max_rows},\
         {current_row_use},{compression},{display_importance},{drag},0",
        compression = root_code(
            tail.behavior_on_horizontal_compression,
            &[("MoveItemsByImportance", "2")],
            "0",
        )?,
        group_horizontal = root_code(
            tail.group_horizontal_align,
            &[("Left", "0"), ("Center", "1"), ("Right", "2")],
            "3",
        )?,
        group_vertical = root_code(
            tail.group_vertical_align,
            &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
            "3",
        )?,
        display_importance = tail.display_importance,
        auto_mark = native_tristate(tail.auto_mark_incomplete),
        auto_add = native_tristate(tail.auto_add_incomplete),
        visible = u8::from(tail.visible),
        multiple_choice = u8::from(tail.multiple_choice),
        skip_on_input = native_tristate(tail.skip_on_input),
        extended_tooltip = tail.extended_tooltip,
        first = tail.additions[0],
        second = tail.additions[1],
        third = tail.additions[2],
        auto_max_width = u8::from(tail.auto_max_width),
        max_width = tail.max_width.unwrap_or("0"),
        auto_max_height = u8::from(tail.auto_max_height),
        max_height = tail.max_height.unwrap_or("0"),
        auto_max_rows = u8::from(tail.auto_max_rows_count),
        max_rows = tail.max_rows_count.unwrap_or("0"),
    ))
}

/// One of the three additions a `<Table>` carries, as the body stores it.
///
/// `<SearchStringAddition>`, `<ViewStatusAddition>` and `<SearchControlAddition>`
/// share one twenty-four member frame and differ in three places: member 5,
/// the kind; member 13, the payload, which has its own layout per kind; and
/// member 19's second element, the kind again. Measured over all 32 606
/// additions of ERP УХ, where every own property of every addition is placed.
///
/// Member 4 is a flag that 38 records set, for a `UserVisible` tuple that no
/// property of the source decides -- the same class of fact as the navigator
/// record. It is written as 0, which is what the other 32 568 carry.
pub(crate) struct NativeTableAddition<'a> {
    pub(crate) id: &'a str,
    /// Member 5: search string 0, view status 1, search control 2.
    pub(crate) kind: u8,
    pub(crate) name: &'a str,
    /// Already formatted -- `{1,0}` when the addition names neither.
    pub(crate) title: &'a str,
    pub(crate) tooltip_title: &'a str,
    /// `<Visible>` and `<Enabled>`, on unless the addition turns them off.
    pub(crate) visible: bool,
    pub(crate) enabled: bool,
    /// `<ToolTipRepresentation>`: `None` 1, `Button` 3, `ShowTop` 5.
    pub(crate) tooltip_representation: Option<&'a str>,
    /// Member 13, by kind -- see [`format_search_string_addition_payload`] and
    /// its two siblings.
    pub(crate) payload: &'a str,
    /// Members 15 and 17, the `<ContextMenu>` and the `<ExtendedTooltip>`.
    pub(crate) context_menu: &'a str,
    pub(crate) extended_tooltip: &'a str,
    /// Member 19: the id of the item `<AdditionSource><Item>` names.
    pub(crate) source_item: &'a str,
    /// Member 20: the count of the addition's own children, then a
    /// `<kind uuid>,<record>` pair per child -- the group child grammar (1 of
    /// 1; `0` in 36 349).
    pub(crate) children: &'a str,
    /// Member 21, `<GroupHorizontalAlign>`: `Left` 0, `Right` 2, absent 3.
    pub(crate) group_horizontal_align: Option<&'a str>,
    /// Member 23, the `DisplayImportance` **attribute** -- not a child
    /// element, which is why no census of children finds it.
    pub(crate) display_importance: Option<&'a str>,
}

pub(crate) fn format_table_addition(addition: &NativeTableAddition<'_>) -> Option<String> {
    let tooltip_representation = root_code(
        addition.tooltip_representation,
        &[("None", "1"), ("Button", "3"), ("ShowTop", "5")],
        "0",
    )?;
    let align = root_code(
        addition.group_horizontal_align,
        &[("Left", "0"), ("Right", "2")],
        "3",
    )?;
    let importance = root_code(
        addition.display_importance,
        &[("VeryHigh", "1"), ("VeryLow", "5")],
        "0",
    )?;
    Some(format!(
        "{{5,{{{id},{ns}}},0,0,0,{kind},{name},{title},{tooltip_title},{visible},{enabled},\
         {tooltip_representation},1,{payload},1,{context_menu},1,{extended_tooltip},2,\
         {{{source},{kind}}},{children},{align},3,{importance}}}",
        id = addition.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = addition.kind,
        name = quoted(addition.name),
        title = addition.title,
        tooltip_title = addition.tooltip_title,
        visible = u8::from(addition.visible),
        enabled = u8::from(addition.enabled),
        payload = addition.payload,
        context_menu = addition.context_menu,
        extended_tooltip = addition.extended_tooltip,
        source = addition.source_item,
        children = addition.children,
    ))
}

/// Member 13 of a `<SearchStringAddition>` -- eleven slots, four of them read.
pub(crate) fn format_search_string_addition_payload(
    width: &str,
    horizontal_stretch: Option<bool>,
    auto_max_width: bool,
    max_width: &str,
) -> String {
    format!(
        "{{1,{width},{stretch},{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},\
         {{0,1,0}},{auto_max_width},{max_width},0}}",
        stretch = native_tristate(horizontal_stretch),
        auto_max_width = u8::from(auto_max_width),
    )
}

/// Member 13 of a `<SearchControlAddition>` -- eleven slots, one of them read.
pub(crate) fn format_search_control_addition_payload(auto_max_width: bool) -> String {
    format!(
        "{{1,0,{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},{{0,1,0}},\
         {auto_max_width},0,0,2}}",
        auto_max_width = u8::from(auto_max_width),
    )
}

/// Member 13 of a `<ViewStatusAddition>` -- sixteen slots, two of them read.
pub(crate) fn format_view_status_addition_payload(
    horizontal_location: Option<&str>,
    auto_max_width: bool,
) -> Option<String> {
    let location = root_code(horizontal_location, &[("Left", "0")], "3")?;
    Some(format!(
        "{{1,0,2,{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{7,3,0,1,100}},\
         {{3,0,{{0}},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},{location},{{0,1,0}},\
         {auto_max_width},0,0}}",
        auto_max_width = u8::from(auto_max_width),
    ))
}

/// A decoration, as the body stores it.
///
/// An `<ExtendedTooltip>` is the commonest record in a form body -- 421 852 of
/// them in ERP УХ -- and it is a decoration like any other, told apart only by
/// where it sits. With the optional functional-options block at member 4
/// lifted, a tooltip's record is **34 members in all 421 849** of them, and
/// **421 844 rebuild byte for byte**: five differ in two members the element
/// does not carry.
///
/// A `<LabelDecoration>` and a `<PictureDecoration>` are the **same record**:
/// what makes them longer is two children a tooltip does not carry, each
/// behind its own flag -- a context menu after the payload, and an extended
/// tooltip of their own after the content. Read as one layout, the writer
/// rebuilds **464 526 of the 464 539** decoration records of ERP УХ.
pub(crate) struct NativeDecorationItem<'a> {
    pub(crate) id: &'a str,
    /// The functional-options block, when the decoration restricts itself.
    pub(crate) functional_options: Option<&'a str>,
    /// Member 5: 0 for a label or a tooltip, 1 for a picture.
    pub(crate) kind: u8,
    pub(crate) name: &'a str,
    /// Already formatted -- the title and the tooltip title.
    pub(crate) title: &'a str,
    pub(crate) tooltip_title: &'a str,
    pub(crate) width: Option<&'a str>,
    pub(crate) height: Option<&'a str>,
    /// `<HorizontalStretch>` and `<VerticalStretch>`: 2 when unnamed.
    pub(crate) horizontal_stretch: Option<bool>,
    pub(crate) vertical_stretch: Option<bool>,
    /// Member 14, `<TextColor>`, and the font, already formatted. The colour
    /// is the text's, not the background's: over the 8 033 label decorations
    /// of ERP УХ that carry one, `web:FireBrick` is stored there as
    /// `{3,2,{44}}` and `style:SpecialTextColor` as `{3,3,{-16}}`.
    pub(crate) text_color: &'a str,
    pub(crate) font: &'a str,
    /// Member 18: the payload that carries the decoration's own properties.
    /// A label and a tooltip carry a nine-member `{5,…}` -- see
    /// `format_label_decoration_payload` -- and a picture a thirteen-member
    /// `{4,…}`, which `format_picture_decoration_payload` writes. The default
    /// below is the `{5,…}` a tooltip carries; a caller that writes a picture
    /// and leaves it would store a payload of the wrong shape.
    pub(crate) payload: &'a str,
    /// The `{1,…}` block that carries what the decoration shows.
    pub(crate) content: &'a str,
    /// `<Enabled>`, on unless the decoration turns it off.
    pub(crate) enabled: bool,
    /// The decoration's context menu, which a tooltip never has.
    pub(crate) context_menu: Option<&'a str>,
    /// `<Visible>`, on unless the decoration turns it off.
    pub(crate) visible: bool,
    /// `<SkipOnInput>`: 2 when the decoration names neither value.
    pub(crate) skip_on_input: Option<bool>,
    /// `<ToolTipRepresentation>`.
    pub(crate) tooltip_representation: Option<&'a str>,
    /// The decoration's own extended tooltip, which a tooltip never has.
    pub(crate) extended_tooltip: Option<&'a str>,
    pub(crate) auto_max_width: bool,
    pub(crate) max_width: Option<&'a str>,
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: Option<&'a str>,
    /// `<GroupHorizontalAlign>` and `<GroupVerticalAlign>`.
    pub(crate) group_horizontal_align: Option<&'a str>,
    pub(crate) group_vertical_align: Option<&'a str>,
    /// Member 34, see [`native_display_importance`].
    pub(crate) display_importance: &'a str,
    /// Member 16, `<Shortcut>`, `{0,0,0}` by default.
    pub(crate) shortcut: &'a str,
}

impl Default for NativeDecorationItem<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            functional_options: None,
            kind: 0,
            name: "",
            title: "{1,0}",
            tooltip_title: "{1,0}",
            width: None,
            height: None,
            horizontal_stretch: None,
            vertical_stretch: None,
            text_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            payload: concat!(
                "{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},",
                "{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}}"
            ),
            content: "{1,{1,0},0}",
            enabled: true,
            context_menu: None,
            visible: true,
            skip_on_input: None,
            tooltip_representation: None,
            extended_tooltip: None,
            auto_max_width: true,
            max_width: None,
            auto_max_height: true,
            max_height: None,
            group_horizontal_align: None,
            group_vertical_align: None,
            display_importance: "0",
            shortcut: "{0,0,0}",
        }
    }
}

/// The `{12,…}` record of a decoration.
pub(crate) fn format_decoration_item(decoration: &NativeDecorationItem<'_>) -> Option<String> {
    let options = match decoration.functional_options {
        Some(block) => format!("1,{block}"),
        None => "0".to_string(),
    };
    let horizontal = root_code(
        decoration.group_horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let vertical = root_code(
        decoration.group_vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    let menu = match decoration.context_menu {
        Some(record) => format!("1,{record}"),
        None => "0".to_string(),
    };
    let tooltip = match decoration.extended_tooltip {
        Some(record) => format!("1,{record}"),
        None => "0".to_string(),
    };
    let tooltip_representation = root_code(
        decoration.tooltip_representation,
        &[
            ("Auto", "0"),
            ("None", "1"),
            ("Balloon", "2"),
            ("Button", "3"),
            ("ShowAuto", "4"),
            ("ShowTop", "5"),
            ("ShowLeft", "6"),
            ("ShowBottom", "7"),
            ("ShowRight", "8"),
        ],
        "0",
    )?;
    Some(format!(
        "{{12,{{{id},{ns}}},0,0,{options},{kind},{name},{title},{tooltip_title},{enabled},\
         {width},{height},{horizontal_stretch},{vertical_stretch},{text_color},{font},\
         {shortcut},1,{payload},{menu},{visible},{skip_on_input},{content},\
         {tooltip_representation},{tooltip},{auto_max_width},{max_width},0,\
         {auto_max_height},{max_height},{horizontal},{vertical},{display_importance},0}}",
        enabled = u8::from(decoration.enabled),
        visible = u8::from(decoration.visible),
        skip_on_input = native_tristate(decoration.skip_on_input),
        id = decoration.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = decoration.kind,
        name = quoted(decoration.name),
        title = decoration.title,
        tooltip_title = decoration.tooltip_title,
        width = decoration.width.unwrap_or("0"),
        height = decoration.height.unwrap_or("0"),
        horizontal_stretch = native_tristate(decoration.horizontal_stretch),
        vertical_stretch = native_tristate(decoration.vertical_stretch),
        text_color = decoration.text_color,
        font = decoration.font,
        payload = decoration.payload,
        content = decoration.content,
        auto_max_width = u8::from(decoration.auto_max_width),
        max_width = decoration.max_width.unwrap_or("0"),
        auto_max_height = u8::from(decoration.auto_max_height),
        max_height = decoration.max_height.unwrap_or("0"),
        display_importance = decoration.display_importance,
        shortcut = decoration.shortcut,
    ))
}

/// The `{3,0,{0},…}` control border member 8 of a label decoration's payload
/// and member 7 of a picture decoration's carry.
///
/// Read by the partition test over all 38 218 label decorations and 8 250
/// picture decorations of both corpora: `<Border>` maps each of its spellings,
/// and its absence, to exactly one stored tuple, with **no impure spelling**.
/// Member 3 is the style code -- `WithoutBorder` 0, `Single` 1, `Embossed` 2,
/// `Indented` 3, `Underline` 4, `Overline` 7, `Double` 200, the same seven
/// `FormControlBorderStyle` already spells -- and member 4 is the `width`
/// attribute, which is `1` in all but eight records of the two corpora.
///
/// Those eight, and the two whose `<Border>` names neither a width nor a style
/// and stores `{3,1,{-18},1,1,0}` -- a six-member tuple of a different shape
/// entirely -- are refused by the caller rather than written with width 1.
pub(crate) fn format_native_control_border(style: Option<&str>, width: &str) -> Option<String> {
    let code = root_code(
        style,
        &[
            ("WithoutBorder", "0"),
            ("Single", "1"),
            ("Embossed", "2"),
            ("Indented", "3"),
            ("Underline", "4"),
            ("Overline", "7"),
            ("Double", "200"),
        ],
        "0",
    )?;
    Some(format!(
        "{{3,0,{{0}},{code},{width},0,{DEFAULT_APPEARANCE_UUID}}}"
    ))
}

/// The `{4,…}` picture reference member 1 of a picture decoration's payload
/// carries.
///
/// Nine members, measured over all 8 094 picture decorations of both corpora
/// whose reference has that arity: member 1 is 1 when the item names a
/// `<Picture>` and 0 when it does not; member 2 is `{0,<the common picture's
/// uuid>}` or `{0}`; members 4 and 5 are the `x` and `y` of
/// `<xr:TransparentPixel>`, `-1` each when the element is absent; and member 6
/// is `<xr:LoadTransparent>`, 0 for `false`, 1 for `true` -- and **1** when
/// there is no picture at all. Members 3, 7 and 8 are `""`, `0` and `""` in
/// every record.
///
/// The 156 references that are ten members hold a `<Picture><Abs>` base64
/// image; the caller refuses them.
pub(crate) fn format_native_item_picture(
    uuid: Option<&str>,
    load_transparent: bool,
    transparent_x: Option<&str>,
    transparent_y: Option<&str>,
) -> String {
    let (present, reference) = match uuid {
        Some(uuid) => ("1", format!("{{0,{uuid}}}")),
        None => ("0", "{0}".to_string()),
    };
    format!(
        "{{4,{present},{reference},\"\",{x},{y},{transparent},0,\"\"}}",
        x = transparent_x.unwrap_or("-1"),
        y = transparent_y.unwrap_or("-1"),
        transparent = u8::from(uuid.is_none() || load_transparent),
    )
}

/// An inline picture: the bytes of a file the export carries beside the form.
///
/// Ten members, not the nine a named picture takes, and member 1 is `3` where
/// a common picture writes 1 and no picture writes 0. Member 6 is
/// `<xr:LoadTransparent>` read straight -- `false` is 0 and `true` is 1, with
/// nothing in between, unlike the common-picture reading where an absent
/// picture also writes 1. Every member partitions over all 225 references of
/// both corpora, and the base64 in member 7 decodes to the named file byte
/// for byte on all 225.
pub(crate) fn format_native_inline_picture(
    bytes: &[u8],
    load_transparent: bool,
    transparent_x: Option<&str>,
    transparent_y: Option<&str>,
) -> String {
    format!(
        "{{4,3,{{0}},\"\",{x},{y},{transparent},{{{{#base64:{data}}}}},0,\"\"}}",
        x = transparent_x.unwrap_or("-1"),
        y = transparent_y.unwrap_or("-1"),
        transparent = u8::from(load_transparent),
        data = crate::module_blob::encode_base64(bytes),
    )
}

/// A picture reference to one of the platform's own pictures.
///
/// `value` is what the name stores -- `{0,<uuid>}` for most of them, a bare
/// negative code for a few. The rest of the reference holds one shape over
/// all 666 of the corpus: present, the transparent pixel when the source
/// names one, and member 6 is **1**, not the 0 a common picture takes --
/// unless the source says `<xr:LoadTransparent>false`, which stores 0.
pub(crate) fn format_native_std_picture(
    value: &str,
    load_transparent: bool,
    transparent_x: Option<&str>,
    transparent_y: Option<&str>,
) -> String {
    format!(
        "{{4,1,{value},\"\",{x},{y},{transparent},0,\"\"}}",
        x = transparent_x.unwrap_or("-1"),
        y = transparent_y.unwrap_or("-1"),
        transparent = u8::from(load_transparent),
    )
}

/// The `{5,…}` payload a `<LabelDecoration>` -- and an `<ExtendedTooltip>`,
/// which is the same record -- carries at member 18.
///
/// Nine members, every one of them named by the partition test over all
/// 38 218 label decorations of the two corpora, each property mapping every
/// spelling **and its absence** to exactly one stored value:
///
/// | member | property | absent | spellings |
/// |---|---|---|---|
/// | 1 | `<Hyperlink>` | 0 | `true` 1 |
/// | 2 | `<HorizontalAlign>` | 0 | `Center` 1, `Right` 2, `Auto` 3 |
/// | 3 | `<VerticalAlign>` | 3 | `Top` 0, `Center` 1, `Bottom` 2 |
/// | 4 | `<TitleHeight>` | 0 | the number itself |
/// | 5 | `<Events>` | `{0,1,0}` | the event block |
/// | 6 | `<BackColor>` | `{3,4,{0}}` | the colour |
/// | 7 | `<BorderColor>` | `{3,4,{0}}` | the colour |
/// | 8 | `<Border>` | style 0, width 1 | see `format_native_control_border` |
///
/// `<HorizontalAlign>Left` and `<VerticalAlign>Auto` are refused: neither is
/// stored anywhere in either corpus, so nothing says what they would write.
///
/// The payload carries **no font**. A decoration's `<Font>` is member 15 of
/// the record, beside `<TextColor>` at member 14; member 5 here, which reads
/// like a reference, is the event block, and the uuid in it is the event's --
/// `11707a99-…` is `Click`, `d710ea07-…` is `URLProcessing`.
///
/// Rebuilt from the source alone, these rules write **36 336 of 36 342** ERP
/// УХ payloads and **1 866 of 1 866** BSP ones byte for byte. The six that
/// differ are stored state the element does not carry: four bind `Click` to an
/// empty handler with no `<Events>` at all, and two store a second event group
/// the source does not name.
pub(crate) struct NativeLabelDecorationPayload<'a> {
    pub(crate) hyperlink: bool,
    pub(crate) horizontal_align: Option<&'a str>,
    pub(crate) vertical_align: Option<&'a str>,
    pub(crate) title_height: Option<&'a str>,
    /// Already formatted -- the event block, the two colours and the border.
    pub(crate) events: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
    pub(crate) border: &'a str,
}

impl Default for NativeLabelDecorationPayload<'_> {
    fn default() -> Self {
        Self {
            hyperlink: false,
            horizontal_align: None,
            vertical_align: None,
            title_height: None,
            events: "{0,1,0}",
            back_color: "{3,4,{0}}",
            border_color: "{3,4,{0}}",
            border: concat!(
                "{3,0,{0},0,1,0,",
                "48312c09-257f-4b29-b280-284dd89efc1e}"
            ),
        }
    }
}

pub(crate) fn format_label_decoration_payload(
    payload: &NativeLabelDecorationPayload<'_>,
) -> Option<String> {
    let horizontal = root_code(
        payload.horizontal_align,
        &[("Center", "1"), ("Right", "2"), ("Auto", "3")],
        "0",
    )?;
    let vertical = root_code(
        payload.vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    Some(format!(
        "{{5,{hyperlink},{horizontal},{vertical},{title_height},{events},{back_color},\
         {border_color},{border}}}",
        hyperlink = u8::from(payload.hyperlink),
        title_height = payload.title_height.unwrap_or("0"),
        events = payload.events,
        back_color = payload.back_color,
        border_color = payload.border_color,
        border = payload.border,
    ))
}

/// The `{4,…}` payload a `<PictureDecoration>` carries at member 18.
///
/// A picture decoration is the **same `{12,…}` record** as a label decoration
/// -- what tells them apart is member 5 of the record and the shape of this
/// payload, which is thirteen members and begins with `4`, not nine beginning
/// with `5`. Writing the label's constant into a picture, which is what this
/// writer did for all 7 222 of ERP УХ, cannot be right for any of them.
///
/// Every member named by the partition test over all 8 250 picture decorations
/// of the two corpora:
///
/// | member | property | absent | spellings |
/// |---|---|---|---|
/// | 1 | `<Picture>` | `{4,0,{0},"",-1,-1,1,0,""}` | see `format_native_item_picture` |
/// | 2 | `<Hyperlink>` | 0 | `true` 1 |
/// | 3 | `<PictureSize>` | 0 | `Stretch` 1, `Proportionally` 2, `AutoSize` 4, `RealSizeIgnoreScale` 5, `AutoSizeIgnoreScale` 6, `ByFontSize` 7 |
/// | 4 | `<Zoomable>` | 0 | `true` 1 |
/// | 5 | `<NonselectedPictureText>` | `{1,0}` | the localized string |
/// | 6 | `<BorderColor>` | `{3,4,{0}}` | the colour |
/// | 7 | `<Border>` | style 0, width 1 | see `format_native_control_border` |
/// | 8 | `<EnableStartDrag>` | 0 | `true` 1 |
/// | 9 | `<EnableDrag>` | 0 | `true` 1 |
/// | 10 | `<Events>` | `{0,1,0}` | the event block |
/// | 11 | `<FileDragMode>` | **1** | `AsFile` **0** |
/// | 12 | `<ImageScale>` | 100 | the number itself |
///
/// `<PictureSize>RealSize` is refused: it is never stored, and although 0 is
/// what an absent `<PictureSize>` writes, nothing says the spelling writes it.
/// Member 11 is inverted the same way the table tail's `<FileDragMode>` is.
///
/// Rebuilt from the source alone these rules write **6 690 of the 6 694** ERP
/// УХ payloads the rules accept and **535 of 535** BSP ones byte for byte. The
/// four that differ are both artefacts of the measurement probe, not of the
/// grammar: it had sorted the three events of those items by name instead of
/// keeping the source's order, and the body reader it used strips newlines,
/// which three of the four carry inside a `<NonselectedPictureText>`. The
/// caller refuses that element anyway, so the writer never reaches them.
pub(crate) struct NativePictureDecorationPayload<'a> {
    /// Already formatted -- the picture, the text, the colour, the border and
    /// the event block.
    pub(crate) picture: &'a str,
    pub(crate) hyperlink: bool,
    pub(crate) picture_size: Option<&'a str>,
    pub(crate) zoomable: bool,
    pub(crate) nonselected_picture_text: &'a str,
    pub(crate) border_color: &'a str,
    pub(crate) border: &'a str,
    pub(crate) enable_start_drag: bool,
    pub(crate) enable_drag: bool,
    pub(crate) events: &'a str,
    /// `<FileDragMode>`, of which only `AsFile` is ever stored, and which
    /// writes 0 where naming nothing writes 1.
    pub(crate) file_drag_mode: Option<&'a str>,
    pub(crate) image_scale: Option<&'a str>,
}

impl Default for NativePictureDecorationPayload<'_> {
    fn default() -> Self {
        Self {
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            hyperlink: false,
            picture_size: None,
            zoomable: false,
            nonselected_picture_text: "{1,0}",
            border_color: "{3,4,{0}}",
            border: concat!(
                "{3,0,{0},0,1,0,",
                "48312c09-257f-4b29-b280-284dd89efc1e}"
            ),
            enable_start_drag: false,
            enable_drag: false,
            events: "{0,1,0}",
            file_drag_mode: None,
            image_scale: None,
        }
    }
}

pub(crate) fn format_picture_decoration_payload(
    payload: &NativePictureDecorationPayload<'_>,
) -> Option<String> {
    let size = root_code(
        payload.picture_size,
        &[
            ("Stretch", "1"),
            ("Proportionally", "2"),
            ("AutoSize", "4"),
            ("RealSizeIgnoreScale", "5"),
            ("AutoSizeIgnoreScale", "6"),
            ("ByFontSize", "7"),
        ],
        "0",
    )?;
    // A decoration that names no drag mode writes 1, not 0.
    let drag = root_code(payload.file_drag_mode, &[("AsFile", "0")], "1")?;
    Some(format!(
        "{{4,{picture},{hyperlink},{size},{zoomable},{text},{border_color},{border},\
         {start_drag},{drag_enabled},{events},{drag},{scale}}}",
        picture = payload.picture,
        hyperlink = u8::from(payload.hyperlink),
        zoomable = u8::from(payload.zoomable),
        text = payload.nonselected_picture_text,
        border_color = payload.border_color,
        border = payload.border,
        start_drag = u8::from(payload.enable_start_drag),
        drag_enabled = u8::from(payload.enable_drag),
        events = payload.events,
        scale = payload.image_scale.unwrap_or("100"),
    ))
}

/// A button, as the body stores it.
///
/// A button's optional functional-options block sits one member **earlier**
/// than a field's: member 3 is the flag and member 4 the block. With it lifted
/// the record is fixed-length, 52 members, in all 77 127 button records of
/// ERP УХ -- and **76 913 of them rebuild byte for byte (99.72%)** from the
/// source alone, the remaining 214 differing only in member 48, which the
/// caller supplies.
///
/// Two properties are written **twice, under two different codings**:
/// `<Type>` is coarse at member 4 (a command-bar hyperlink counts as a
/// command-bar button) and fine at member 46, and `<LocationInCommandBar>` is
/// coarse at member 15 and fine at member 49. Reading either one once leaves
/// tens of thousands of records wrong.
pub(crate) struct NativeButtonItem<'a> {
    pub(crate) id: &'a str,
    /// The functional-options block, when the button restricts itself.
    pub(crate) functional_options: Option<&'a str>,
    /// `<Type>`: `CommandBarButton`, `CommandBarHyperlink`, `UsualButton` or
    /// `Hyperlink`.
    pub(crate) button_type: Option<&'a str>,
    pub(crate) name: &'a str,
    /// Already formatted -- `{1,0}` when the button names no title.
    pub(crate) title: &'a str,
    pub(crate) enabled: bool,
    /// The command the button runs, as `{<id>,<command namespace>}`.
    pub(crate) command: &'a str,
    /// The button's data path, `{0}` when it has none.
    pub(crate) data_path: &'a str,
    /// `<Representation>`: `Text`, `Picture` or `PictureAndText`.
    pub(crate) representation: Option<&'a str>,
    pub(crate) default_button: bool,
    pub(crate) default_item: bool,
    /// `<LocationInCommandBar>`: `InCommandBar`, `InAdditionalSubmenu` or
    /// `InCommandBarAndInAdditionalSubmenu`.
    pub(crate) location_in_command_bar: Option<&'a str>,
    pub(crate) width: Option<&'a str>,
    pub(crate) height: Option<&'a str>,
    pub(crate) title_height: Option<&'a str>,
    /// The back, text and border colours and the font, already formatted.
    pub(crate) appearance: [&'a str; 4],
    pub(crate) check: bool,
    /// The picture, already formatted.
    pub(crate) picture: &'a str,
    pub(crate) visible: bool,
    /// `<SkipOnInput>`: 2 when the button names neither value.
    pub(crate) skip_on_input: Option<bool>,
    /// `<ToolTipRepresentation>`.
    pub(crate) tooltip_representation: Option<&'a str>,
    pub(crate) extended_tooltip: &'a str,
    pub(crate) auto_max_width: bool,
    pub(crate) max_width: Option<&'a str>,
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: Option<&'a str>,
    pub(crate) horizontal_stretch: bool,
    pub(crate) vertical_stretch: bool,
    /// `<GroupHorizontalAlign>` and `<GroupVerticalAlign>`.
    pub(crate) group_horizontal_align: Option<&'a str>,
    pub(crate) group_vertical_align: Option<&'a str>,
    /// `<RepresentationInContextMenu>`: `None`, `AdditionalInContextMenu` or
    /// `OnlyInContextMenu`.
    pub(crate) representation_in_context_menu: Option<&'a str>,
    /// `<Shape>`: `Usual` or `Oval`.
    pub(crate) shape: Option<&'a str>,
    /// `<ShapeRepresentation>`: `Always`, `WhenActive` or `None`.
    pub(crate) shape_representation: Option<&'a str>,
    /// `<PictureLocation>`: `Left` or `Right`.
    pub(crate) picture_location: Option<&'a str>,
    /// Member 48, which the button's own element does not carry: 214 records
    /// of the corpus name something there and the rest write 0.
    pub(crate) forty_eighth: &'a str,
    pub(crate) command_uniqueness: bool,
    /// Member 23, `<Shortcut>`, and member 33, the command's `<Parameter>`.
    pub(crate) shortcut: &'a str,
    pub(crate) parameter: &'a str,
}

impl Default for NativeButtonItem<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            functional_options: None,
            button_type: None,
            name: "",
            title: "{1,0}",
            enabled: true,
            command: "{0}",
            data_path: "{0}",
            representation: None,
            default_button: false,
            default_item: false,
            location_in_command_bar: None,
            width: None,
            height: None,
            title_height: None,
            appearance: ["{3,4,{0}}", "{3,4,{0}}", "{3,4,{0}}", "{7,3,0,1,100}"],
            check: false,
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            visible: true,
            skip_on_input: None,
            tooltip_representation: None,
            extended_tooltip: "",
            auto_max_width: true,
            max_width: None,
            auto_max_height: true,
            max_height: None,
            horizontal_stretch: false,
            vertical_stretch: false,
            group_horizontal_align: None,
            group_vertical_align: None,
            representation_in_context_menu: None,
            shape: None,
            shape_representation: None,
            picture_location: None,
            forty_eighth: "0",
            command_uniqueness: true,
            shortcut: "{0,0,0}",
            parameter: "{\"U\"}",
        }
    }
}

/// The `{31,…}` record of a button.
pub(crate) fn format_button_item(button: &NativeButtonItem<'_>) -> Option<String> {
    let options = match button.functional_options {
        Some(block) => format!("1,{block}"),
        None => "0".to_string(),
    };
    let coarse_type = root_code(
        button.button_type,
        &[
            ("CommandBarButton", "0"),
            ("CommandBarHyperlink", "0"),
            ("UsualButton", "1"),
            ("Hyperlink", "2"),
        ],
        "0",
    )?;
    let fine_type = root_code(
        button.button_type,
        &[
            ("CommandBarButton", "0"),
            ("UsualButton", "1"),
            ("Hyperlink", "2"),
            ("CommandBarHyperlink", "3"),
        ],
        "0",
    )?;
    let coarse_location = root_code(
        button.location_in_command_bar,
        &[
            ("InAdditionalSubmenu", "0"),
            ("InCommandBar", "1"),
            ("InCommandBarAndInAdditionalSubmenu", "1"),
        ],
        "2",
    )?;
    let fine_location = root_code(
        button.location_in_command_bar,
        &[
            ("InAdditionalSubmenu", "1"),
            ("InCommandBar", "2"),
            ("InCommandBarAndInAdditionalSubmenu", "3"),
        ],
        "0",
    )?;
    let representation = root_code(
        button.representation,
        &[("Text", "0"), ("Picture", "1"), ("PictureAndText", "2")],
        "3",
    )?;
    let tooltip_representation = root_code(
        button.tooltip_representation,
        &[
            ("Auto", "0"),
            ("None", "1"),
            ("Balloon", "2"),
            ("Button", "3"),
            ("ShowAuto", "4"),
            ("ShowTop", "5"),
            ("ShowLeft", "6"),
            ("ShowBottom", "7"),
            ("ShowRight", "8"),
        ],
        "0",
    )?;
    let group_horizontal = root_code(
        button.group_horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let group_vertical = root_code(
        button.group_vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    let in_context_menu = root_code(
        button.representation_in_context_menu,
        &[
            ("None", "0"),
            ("AdditionalInContextMenu", "1"),
            ("OnlyInContextMenu", "2"),
        ],
        "3",
    )?;
    let shape = root_code(button.shape, &[("Usual", "1"), ("Oval", "2")], "0")?;
    let shape_representation = root_code(
        button.shape_representation,
        &[("Always", "1"), ("WhenActive", "2"), ("None", "3")],
        "0",
    )?;
    let picture_location = root_code(
        button.picture_location,
        &[("Left", "1"), ("Right", "2")],
        "0",
    )?;
    Some(format!(
        "{{31,{{{id},{ns}}},0,{options},{coarse_type},{name},{title},{enabled},{command},\
         {data_path},{representation},{default_button},0,{default_item},2,{coarse_location},\
         {width},{height},{title_height},{back},{text},{border},{font},{shortcut},{check},\
         {picture},{visible},{{\"Pattern\"}},\"\",{skip_on_input},{tooltip_representation},1,\
         {extended_tooltip},{parameter},{auto_max_width},{max_width},0,{auto_max_height},\
         {max_height},{horizontal_stretch},{vertical_stretch},{group_horizontal},\
         {group_vertical},{in_context_menu},{shape},{shape_representation},{fine_type},\
         {picture_location},{forty_eighth},{fine_location},{command_uniqueness},0}}",
        id = button.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(button.name),
        title = button.title,
        enabled = u8::from(button.enabled),
        command = button.command,
        data_path = button.data_path,
        default_button = u8::from(button.default_button),
        default_item = u8::from(button.default_item),
        width = button.width.unwrap_or("0"),
        height = button.height.unwrap_or("0"),
        title_height = button.title_height.unwrap_or("0"),
        back = button.appearance[0],
        text = button.appearance[1],
        border = button.appearance[2],
        font = button.appearance[3],
        check = u8::from(button.check),
        picture = button.picture,
        visible = u8::from(button.visible),
        skip_on_input = native_tristate(button.skip_on_input),
        extended_tooltip = button.extended_tooltip,
        auto_max_width = u8::from(button.auto_max_width),
        max_width = button.max_width.unwrap_or("0"),
        auto_max_height = u8::from(button.auto_max_height),
        max_height = button.max_height.unwrap_or("0"),
        horizontal_stretch = u8::from(button.horizontal_stretch),
        vertical_stretch = u8::from(button.vertical_stretch),
        forty_eighth = button.forty_eighth,
        command_uniqueness = u8::from(button.command_uniqueness),
        shortcut = button.shortcut,
        parameter = button.parameter,
    ))
}

/// A form attribute, as the body stores it.
///
/// The record is fourteen members, then the attribute's columns, then two more
/// blocks -- so it is `16 + <column count>` members long and member 13 is that
/// count. Measured over the 102 895 attribute records of ERP УХ: the shape
/// holds in 100 740 of them, and in all 94 031 whose last two blocks are empty
/// the fourteen fixed members rebuild byte for byte.
///
/// Six members name configuration objects rather than spellings of a property,
/// so the caller supplies them already formatted: the title, the type pattern,
/// and members 6 to 9. Everything else is read from the source:
///
/// - member 10 is `<MainAttribute>`;
/// - member 11 is `<SavedData>`;
/// - member 12 is `<FillCheck>`, 1 for `ShowError`.
///
/// Members 6 to 9 used to be named two apart from what they hold -- 6 and 7
/// were called the `<UseAlways>` pair and 8 and 9 the `<View>`/`<Edit>` one.
/// Joined over the 141 723 attributes of both corpora, each of the four is a
/// pure partition on the absence of one element: 6 is `<View>`, 7 is
/// `<Edit>`, 8 is `<UseAlways>` and 9 is `<Save>`. The defaults below are
/// what an attribute that names none of them stores -- 141 493, 141 431,
/// 137 363 and 138 968 records with no exception -- so the values were right
/// all along and only the names were out of step.
pub(crate) struct NativeFormAttribute<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    /// Already formatted -- `{1,0}` for an attribute with no title.
    pub(crate) title: &'a str,
    /// Already formatted -- `{"Pattern"}` for an attribute the form does not
    /// type, and the pattern with its reference list for one it does.
    pub(crate) type_pattern: &'a str,
    /// Member 6, `<View>`, and member 7, `<Edit>` -- `{0,{0,{"B",1},0}}` when
    /// the attribute restricts nothing.
    pub(crate) view: &'a str,
    pub(crate) edit: &'a str,
    /// Member 8, `<UseAlways>`, `{0,0}` when the attribute names none.
    pub(crate) use_always: &'a str,
    /// Member 9, `<Save>`, `{0,0}` when the attribute names none. See
    /// [`format_form_attribute_save`].
    pub(crate) save: &'a str,
    pub(crate) main_attribute: bool,
    pub(crate) saved_data: bool,
    /// `<FillCheck>`: true for `ShowError`.
    pub(crate) fill_check: bool,
    /// The attribute's columns, each a `{5,…}` record, already formatted.
    pub(crate) columns: &'a [String],
    /// The two blocks that close the record, `{0,0}` in 94 031 of the 100 740
    /// records whose shape holds; the rest name a type or an object.
    pub(crate) trailing: [&'a str; 2],
}

impl Default for NativeFormAttribute<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            name: "",
            title: "{1,0}",
            type_pattern: "{\"Pattern\"}",
            view: "{0,{0,{\"B\",1},0}}",
            edit: "{0,{0,{\"B\",1},0}}",
            use_always: "{0,0}",
            save: "{0,0}",
            main_attribute: false,
            saved_data: false,
            fill_check: false,
            columns: &[],
            trailing: ["{0,0}", "{0,0}"],
        }
    }
}

/// The `{9,…}` record of a form attribute.
pub(crate) fn format_form_attribute(attribute: &NativeFormAttribute<'_>) -> String {
    let mut columns = String::new();
    for column in attribute.columns {
        columns.push(',');
        columns.push_str(column);
    }
    format!(
        "{{9,{{{id}}},0,{name},{title},{type_pattern},{view},{edit},\
         {use_always},{save},{main},{saved},{fill_check},{count}{columns},{first},{second}}}",
        id = attribute.id,
        name = quoted(attribute.name),
        title = attribute.title,
        type_pattern = attribute.type_pattern,
        view = attribute.view,
        edit = attribute.edit,
        use_always = attribute.use_always,
        save = attribute.save,
        main = u8::from(attribute.main_attribute),
        saved = u8::from(attribute.saved_data),
        fill_check = u8::from(attribute.fill_check),
        count = attribute.columns.len(),
        first = attribute.trailing[0],
        second = attribute.trailing[1],
    )
}

/// Member 9 of the `{9,…}` record: an attribute's `<Save>`.
///
/// ```text
/// {0, N, <path>, <path>, …}
/// ```
///
/// `N` is the number of `<Field>` children -- 2 755 of 2 755 over both
/// corpora, from 1 to 55 -- and each path follows the `<DataPath>` grammar
/// with the attribute's own leading segment removed: `{0}` for a field that
/// is the attribute's own name, `{1,{0,<uuid>}}` for one further segment.
///
/// **The list is sorted, not in XML order.** Every one of the 99 multi-entry
/// lists of both corpora is ordered by segment count and then by the segment
/// text, and the 65 that are all uuid are in ascending uuid order with no
/// exception. Pairing the nth `<Field>` with the nth entry agrees on the set
/// and silently swaps the uuids.
pub(crate) fn format_form_attribute_save(paths: &[String]) -> String {
    let mut sorted = paths.to_vec();
    // By the segment count, then segment by segment: the first member as a
    // number, the rest as text -- `{1,{-30}}` before `{1,{-20}}`, `{1,{2}}`
    // before `{1,{17}}` (271 of 271 lists).
    let key = |path: &String| {
        let fields = top_level_braced_fields(path);
        let count = fields.first().and_then(|value| value.parse::<i64>().ok()).unwrap_or(0);
        let segments = fields
            .iter()
            .skip(1)
            .map(|segment| {
                let inner = top_level_braced_fields(segment);
                let first = inner.first().and_then(|value| value.parse::<i64>().ok()).unwrap_or(0);
                (first, inner.get(1..).map(|rest| rest.join(",")).unwrap_or_default())
            })
            .collect::<Vec<_>>();
        (count, segments)
    };
    sorted.sort_by(|a, b| key(a).cmp(&key(b)).then_with(|| a.cmp(b)));
    let mut out = format!("{{0,{}", sorted.len());
    for path in sorted {
        out.push(',');
        out.push_str(&path);
    }
    out.push('}');
    out
}

/// The top-level members of one `{…}` value, braces removed.
fn top_level_braced_fields(text: &str) -> Vec<String> {
    let text = text.trim();
    let Some(inner) = text.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')) else {
        return vec![text.to_string()];
    };
    let mut fields = Vec::new();
    let mut depth = 0i32;
    let mut quoted = false;
    let mut current = String::new();
    for character in inner.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                current.push(character);
            }
            '{' if !quoted => {
                depth += 1;
                current.push(character);
            }
            '}' if !quoted => {
                depth -= 1;
                current.push(character);
            }
            ',' if !quoted && depth == 0 => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// One `<Column>` of a value table or a value tree, as the body stores it.
///
/// Ten members in all 143 454 columns of ERP УХ -- the record never recurses,
/// because a column carries no columns of its own:
///
/// ```text
/// {5,<id>,0,<"name">,<title>,<type pattern>,<view>,<edit>,<functional options>,<fill check>}
/// ```
///
/// Member 1 is the `<Column id="…">` attribute verbatim and member 2 the
/// literal 0; members 4 and 5 follow the attribute's own grammar, the title
/// down to `{1,0}` when there is none and the type pattern down to
/// `{"Pattern"}` for the 1 464 columns whose `<Type>` is empty.
///
/// Members 6, 7 and 8 name configuration objects when the column says
/// something -- member 8's bag carries a functional-option uuid only the
/// configuration knows -- so the caller supplies them, and a column that names
/// `<View>`, `<Edit>` or `<FunctionalOptions>` refuses the form rather than
/// take the default. The defaults here are what the column stores when it
/// says nothing, which is 143 448, 143 446 and 142 594 of the 143 454.
///
/// Member 9 is `<FillCheck>`: a pure partition, 143 448 absent to 0 and the
/// six `ShowError` to 1.
pub(crate) struct NativeFormAttributeColumn<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    /// Already formatted -- `{1,0}` for a column with no title.
    pub(crate) title: &'a str,
    /// Already formatted -- `{"Pattern"}` for a column the form does not type.
    pub(crate) type_pattern: &'a str,
    /// The `<View>` and `<Edit>` restrictions.
    pub(crate) restrictions: [&'a str; 2],
    /// `<FunctionalOptions>`, `{0,0}` when the column names none.
    pub(crate) functional_options: &'a str,
    /// `<FillCheck>`: true for `ShowError`.
    pub(crate) fill_check: bool,
}

impl Default for NativeFormAttributeColumn<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            name: "",
            title: "{1,0}",
            type_pattern: "{\"Pattern\"}",
            restrictions: ["{0,{0,{\"B\",1},0}}", "{0,{0,{\"B\",1},0}}"],
            functional_options: "{0,0}",
            fill_check: false,
        }
    }
}

/// The `{5,…}` record of one column of a form attribute.
pub(crate) fn format_form_attribute_column(column: &NativeFormAttributeColumn<'_>) -> String {
    format!(
        "{{5,{id},0,{name},{title},{type_pattern},{view},{edit},{functional_options},{fill_check}}}",
        id = column.id,
        name = quoted(column.name),
        title = column.title,
        type_pattern = column.type_pattern,
        view = column.restrictions[0],
        edit = column.restrictions[1],
        functional_options = column.functional_options,
        fill_check = u8::from(column.fill_check),
    )
}

/// The uuid a `v8:TypeDescription` value is stored under.
///
/// Constant in all 2 140 value-list attributes of ERP УХ that carry a
/// `<Settings>` element type.
const FORM_TYPE_DESCRIPTION_TYPE_UUID: &str = "f5c65050-3bbb-11d5-b988-0050bae0a95d";

/// A value list's element type, as `NativeFormAttribute::trailing[0]`.
///
/// `v8:ValueListType` spells its element type as
/// `<Settings xsi:type="v8:TypeDescription">`, and the body writes it as a
/// one-key bag whose value is a TypeDescription carrying the same
/// `{"Pattern",…}` grammar an attribute's own `<Type>` uses. All 2 140 share
/// this prefix; the 238 value lists with no `<Settings>` store `{0,0}`
/// instead, which is the field's default.
pub(crate) fn format_form_value_list_element_type(type_pattern: &str) -> String {
    format!("{{0,1,\"ElementType\",{{\"#\",{FORM_TYPE_DESCRIPTION_TYPE_UUID},{type_pattern}}}}}")
}

/// A form command, as the body stores it.
///
/// Nineteen members, told apart from an attribute by the namespace in its id
/// tuple. Measured over the 61 228 command records of ERP УХ: 60 983 rebuild
/// byte for byte (99.60%), with the action, the picture and the functional
/// options supplied by the caller because they name configuration objects.
///
/// `<CurrentRowUse>` is written in **two** places and they do not agree:
/// member 13 is 0 only for `Use`, while member 18 is 0 for `Use`, 1 for
/// `DontUse` and 2 when the command says nothing. Reading either alone leaves
/// 61 057 records wrong.
///
/// The 245 records still unaccounted for name something in member 14 that the
/// command's own element does not carry -- values from 1 to 325, which look
/// like a reference into the form. The caller can pass it; the default is the
/// 0 that 60 983 records write.
pub(crate) struct NativeFormCommand<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    /// Already formatted -- `{1,0}` when the command names none.
    pub(crate) title: &'a str,
    pub(crate) tooltip: &'a str,
    /// The `<UseAlways>` block, `{0,{0,{"B",1},0}}` when it restricts nothing.
    pub(crate) use_always: &'a str,
    /// The picture's index tuple and the picture itself.
    pub(crate) picture_index: &'a str,
    pub(crate) picture: &'a str,
    /// `<Action>`: the procedure the command runs.
    pub(crate) action: &'a str,
    /// `<Representation>`: `Text`, `Picture`, `TextPicture` or `Auto`.
    pub(crate) representation: Option<&'a str>,
    pub(crate) modifies_saved_data: bool,
    /// The functional options, `{0,0}` when there are none.
    pub(crate) functional_options: &'a str,
    /// `<CurrentRowUse>`: `Use`, `DontUse` or `Auto`.
    pub(crate) current_row_use: Option<&'a str>,
    /// Member 14, which the command's element does not carry.
    pub(crate) fourteenth: &'a str,
}

impl Default for NativeFormCommand<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            name: "",
            title: "{1,0}",
            tooltip: "{1,0}",
            use_always: "{0,{0,{\"B\",1},0}}",
            picture_index: "{0,0,0}",
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            action: "",
            representation: None,
            modifies_saved_data: false,
            functional_options: "{0,0}",
            current_row_use: None,
            fourteenth: "0",
        }
    }
}

/// The `{9,…}` record of a form command.
pub(crate) fn format_form_command(command: &NativeFormCommand<'_>) -> Option<String> {
    let representation = root_code(
        command.representation,
        &[
            ("Text", "0"),
            ("Picture", "1"),
            ("TextPicture", "2"),
            ("Auto", "3"),
        ],
        "3",
    )?;
    let row_use_early = match command.current_row_use {
        Some("Use") => "0",
        Some("DontUse") | Some("Auto") | None => "1",
        Some(_) => return None,
    };
    let row_use_late = root_code(
        command.current_row_use,
        &[("Use", "0"), ("DontUse", "1"), ("Auto", "2")],
        "2",
    )?;
    Some(format!(
        "{{9,{{{id},{ns}}},{name},{title},{tooltip},{use_always},{picture_index},{picture},\
         {action},{representation},{modifies},0,{options},{row_use_early},{fourteenth},1,0,0,\
         {row_use_late}}}",
        id = command.id,
        ns = FORM_COMMAND_NAMESPACE_UUID,
        name = quoted(command.name),
        title = command.title,
        tooltip = command.tooltip,
        use_always = command.use_always,
        picture_index = command.picture_index,
        picture = command.picture,
        action = quoted(command.action),
        modifies = u8::from(command.modifies_saved_data),
        options = command.functional_options,
        fourteenth = command.fourteenth,
    ))
}

/// The `{0,…}` record of a form parameter.
///
/// Four members in all 24 863 parameter records of ERP УХ, and all 24 863
/// rebuild byte for byte: the name, the type pattern the caller supplies, and
/// whether `<KeyParameter>` is set.
pub(crate) fn format_form_parameter(name: &str, type_pattern: &str, key: bool) -> String {
    format!(
        "{{0,{name},{type_pattern},{key}}}",
        name = quoted(name),
        key = u8::from(key),
    )
}

/// The `{29,…}` payload of a usual group, member by member.
///
/// Twenty-four of its twenty-nine members carry an XML property, each named by
/// the partition test over all 61 256 usual groups of ERP УХ: every spelling
/// of the property, and its absence, maps to exactly one stored value.
///
/// Three properties are written more than once under different codings, and
/// the later reading is always the finer one. `<Group>` is written three
/// times -- slot 1 tells `Vertical` from the rest, slot 22 adds "the group
/// names none", and slot 27 also tells `AlwaysHorizontal` from `Horizontal`.
/// `<Behavior>` likewise: slot 10 tells `Usual` from the rest, slot 24 names
/// which, and slot 28 also tells "names none" from `Usual`.
///
/// Five members carry something no scalar property gives -- the picture, the
/// format, the two colours and the associated table element -- and the writer
/// takes those from its caller.
pub(crate) struct NativeUsualGroupPayload<'a> {
    /// Slots 1, 22 and 27.
    pub(crate) group: Option<&'a str>,
    /// Slots 10, 24 and 28.
    pub(crate) behavior: Option<&'a str>,
    /// Slot 2, `<ChildItemsWidth>`.
    pub(crate) child_items_width: Option<&'a str>,
    /// Slot 3, `<Representation>`, of which "names none" is its own value.
    pub(crate) representation: Option<&'a str>,
    /// Slot 4, `<ShowTitle>`, on unless the group turns it off.
    pub(crate) show_title: bool,
    /// Slot 5, `<TitleDataPath>`, `{0}` by default.
    pub(crate) title_data_path: &'a str,
    /// Slot 6, `<Format>`, `{1,0}` by default.
    pub(crate) format: &'a str,
    /// Slot 9, `<BackColor>`, `{3,4,{0}}` by default.
    pub(crate) back_color: &'a str,
    /// Slot 11, `<ControlRepresentation>`: `Picture` 1.
    pub(crate) control_representation: Option<&'a str>,
    /// Slot 12, `<Collapsed>`.
    pub(crate) collapsed: bool,
    /// Slot 13, `<ShowLeftMargin>`, on unless the group turns it off.
    pub(crate) show_left_margin: bool,
    /// Slot 14, `<CollapsedRepresentationTitle>`, `{1,0}` by default.
    pub(crate) collapsed_representation_title: &'a str,
    /// Slots 15 and 16, `<HorizontalSpacing>` and `<VerticalSpacing>`.
    pub(crate) horizontal_spacing: Option<&'a str>,
    pub(crate) vertical_spacing: Option<&'a str>,
    /// Slots 17 and 18, `<HorizontalAlign>` and `<VerticalAlign>`, each 3 when
    /// the group names none.
    pub(crate) horizontal_align: Option<&'a str>,
    pub(crate) vertical_align: Option<&'a str>,
    /// Slot 19, `<ThroughAlign>`: `Use` 0, `DontUse` 1, absent 2.
    pub(crate) through_align: Option<&'a str>,
    /// Slot 20, `<ChildrenAlign>`.
    pub(crate) children_align: Option<&'a str>,
    /// Slot 21, `<United>`, on unless the group turns it off.
    pub(crate) united: bool,
    /// Slot 23, `<HiddenStateTitleBackColor>`, `{3,4,{0}}` by default.
    pub(crate) hidden_state_title_back_color: &'a str,
    /// Slot 25, `<CurrentRowUse>`: `Use` 0, `DontUse` 1, absent 2.
    pub(crate) current_row_use: Option<&'a str>,
    /// Slot 26, the id of the item `<AssociatedTableElementId>` names, 0 when
    /// the group names none.
    pub(crate) associated_table_element_id: &'a str,
}

impl NativeUsualGroupPayload<'_> {
    /// What a group that names none of these carries.
    pub(crate) const fn plain() -> Self {
        Self {
            group: None,
            behavior: None,
            child_items_width: None,
            representation: None,
            show_title: true,
            title_data_path: "{0}",
            format: "{1,0}",
            back_color: "{3,4,{0}}",
            control_representation: None,
            collapsed: false,
            show_left_margin: true,
            collapsed_representation_title: "{1,0}",
            horizontal_spacing: None,
            vertical_spacing: None,
            horizontal_align: None,
            vertical_align: None,
            through_align: None,
            children_align: None,
            united: true,
            hidden_state_title_back_color: "{3,4,{0}}",
            current_row_use: None,
            associated_table_element_id: "0",
        }
    }
}

pub(crate) fn format_usual_group_payload(payload: &NativeUsualGroupPayload<'_>) -> Option<String> {
    // `<Group>` under its three codings.
    let arrangement = root_code(
        payload.group,
        &[
            ("Vertical", "0"),
            ("Horizontal", "1"),
            ("AlwaysHorizontal", "1"),
        ],
        "1",
    )?;
    let arrangement_named = root_code(
        payload.group,
        &[
            ("Vertical", "0"),
            ("Horizontal", "1"),
            ("AlwaysHorizontal", "1"),
        ],
        "2",
    )?;
    let arrangement_tail = root_code(
        payload.group,
        &[
            ("Vertical", "0"),
            ("Horizontal", "1"),
            ("AlwaysHorizontal", "3"),
        ],
        "2",
    )?;
    // `<Behavior>` under its three codings.
    let collapsible = root_code(
        payload.behavior,
        &[("Usual", "0"), ("Collapsible", "1"), ("PopUp", "1")],
        "1",
    )?;
    let behavior = root_code(
        payload.behavior,
        &[("Usual", "0"), ("Collapsible", "1"), ("PopUp", "2")],
        "0",
    )?;
    let behavior_tail = root_code(
        payload.behavior,
        &[("Usual", "0"), ("Collapsible", "1"), ("PopUp", "2")],
        "3",
    )?;
    let child_items_width = root_code(
        payload.child_items_width,
        &[
            ("Equal", "1"),
            ("LeftWide", "2"),
            ("LeftWidest", "3"),
            ("LeftNarrow", "4"),
            ("LeftNarrowest", "5"),
        ],
        "0",
    )?;
    let separation = root_code(
        payload.representation,
        &[
            ("None", "0"),
            ("StrongSeparation", "1"),
            ("NormalSeparation", "3"),
        ],
        "2",
    )?;
    let control_representation =
        root_code(payload.control_representation, &[("Picture", "1")], "0")?;
    let spacing = |value| {
        root_code(
            value,
            &[
                ("None", "1"),
                ("Half", "2"),
                ("Single", "3"),
                ("OneAndHalf", "4"),
                ("Double", "5"),
            ],
            "0",
        )
    };
    let horizontal_spacing = spacing(payload.horizontal_spacing)?;
    let vertical_spacing = spacing(payload.vertical_spacing)?;
    let horizontal_align = root_code(
        payload.horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let vertical_align = root_code(
        payload.vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    let through_align = root_code(payload.through_align, &[("Use", "0"), ("DontUse", "1")], "2")?;
    let children_align = root_code(
        payload.children_align,
        &[
            ("None", "1"),
            ("ItemsLeftTitlesLeft", "2"),
            ("ItemsRightTitlesLeft", "3"),
            ("ItemsLeftTitlesRight", "4"),
            ("ItemsRightTitlesRight", "5"),
            ("TitlesLeftDataAuto", "6"),
        ],
        "0",
    )?;
    let current_row_use = root_code(
        payload.current_row_use,
        &[("Use", "0"), ("DontUse", "1")],
        "2",
    )?;
    Some(format!(
        "{{29,{arrangement},{child_items_width},{separation},{show_title},{title_data_path},{format},{PATTERN},\"\",{back_color},{collapsible},{control_representation},{collapsed},{show_left_margin},{collapsed_representation_title},{horizontal_spacing},{vertical_spacing},{horizontal_align},{vertical_align},{through_align},{children_align},{united},{arrangement_named},{hidden},{behavior},{current_row_use},{associated},{arrangement_tail},{behavior_tail}}}",
        PATTERN = "{\"Pattern\"}",
        show_title = u8::from(payload.show_title),
        title_data_path = payload.title_data_path,
        format = payload.format,
        back_color = payload.back_color,
        collapsed = u8::from(payload.collapsed),
        show_left_margin = u8::from(payload.show_left_margin),
        collapsed_representation_title = payload.collapsed_representation_title,
        united = u8::from(payload.united),
        hidden = payload.hidden_state_title_back_color,
        associated = payload.associated_table_element_id,
    ))
}

/// The `{2,…}` payload of a button group.
///
/// Four members, one of which carries `<Representation>`: `Usual` 1,
/// `Compact` 2, and 0 when the group names none. All 21 651 of the corpus are
/// reproduced exactly.
pub(crate) fn format_button_group_payload(
    command_source: &str,
    representation: Option<&str>,
) -> Option<String> {
    let representation = root_code(representation, &[("Usual", "1"), ("Compact", "2")], "0")?;
    Some(format!("{{2,{command_source},2,{representation}}}"))
}

/// The `{1,…}` payload of a command bar.
///
/// Three members, one of which carries `<HorizontalLocation>`. All 3 233 of
/// the corpus are reproduced exactly.
pub(crate) fn format_command_bar_payload(
    horizontal_location: Option<&str>,
    command_source: &str,
) -> Option<String> {
    let location = root_code(
        horizontal_location,
        &[("Center", "1"), ("Right", "2"), ("Auto", "3")],
        "0",
    )?;
    Some(format!("{{1,{location},{command_source}}}"))
}

/// The `{4,…}` payload of a `<Pages>` group.
///
/// Six members: two carry `<PagesRepresentation>` -- the same value in both,
/// except that a group naming none carries 1 in the first and 6 in the second
/// -- and one carries `<AssociatedTableElementId>`. All 4 329 of the corpus
/// are reproduced exactly.
pub(crate) fn format_pages_payload(
    representation: Option<&str>,
    events: &str,
    associated_table_element_id: &str,
) -> Option<String> {
    const CODES: &[(&str, &str)] = &[
        ("None", "0"),
        ("TabsOnTop", "1"),
        ("TabsOnBottom", "2"),
        ("TabsOnLeftHorizontal", "3"),
        ("TabsOnRightHorizontal", "4"),
        ("Swipe", "5"),
    ];
    let first = root_code(representation, CODES, "1")?;
    let second = root_code(representation, CODES, "6")?;
    Some(format!(
        "{{4,{first},{events},2,{associated_table_element_id},{second}}}"
    ))
}

/// The `{7,…}` payload of a `<Popup>`.
///
/// Nine members, four of which carry an XML property: `<Representation>`,
/// which is 3 when the popup names none, `<Shape>`, `<ShapeRepresentation>`
/// and the two colours. All 10 642 of the corpus are reproduced exactly.
pub(crate) struct NativePopupPayload<'a> {
    pub(crate) picture: &'a str,
    pub(crate) command_source: &'a str,
    pub(crate) representation: Option<&'a str>,
    pub(crate) shape: Option<&'a str>,
    pub(crate) shape_representation: Option<&'a str>,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
}

impl NativePopupPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            command_source: "{0}",
            representation: None,
            shape: None,
            shape_representation: None,
            back_color: "{3,4,{0}}",
            border_color: "{3,4,{0}}",
        }
    }
}

/// The `{3,…}` payload of an `<HTMLDocumentField>`, thirteen members.
///
/// Every member is a constant over all 222 records of both corpora or passes
/// the partition test outright. Members 3 and 4 are the `border_color_option`
/// and `document_field_output` slots the reader already claims for this kind,
/// so the two directions agree slot for slot.
pub(crate) struct NativeHtmlDocumentPayload<'a> {
    pub(crate) width: &'a str,
    pub(crate) height: &'a str,
    pub(crate) border_color: &'a str,
    pub(crate) output: Option<&'a str>,
    pub(crate) events: &'a str,
    pub(crate) auto_max_width: bool,
    pub(crate) max_width: &'a str,
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: &'a str,
    pub(crate) horizontal_stretch: bool,
    pub(crate) vertical_stretch: bool,
}

pub(crate) fn format_html_document_payload(
    payload: &NativeHtmlDocumentPayload<'_>,
) -> Option<String> {
    let output = root_code(payload.output, &[("Enable", "1"), ("Disable", "2")], "0")?;
    Some(format!(
        "{{3,{width},{height},{border_color},{output},{events},{auto_width},{max_width},0,\
         {auto_height},{max_height},{horizontal},{vertical}}}",
        width = payload.width,
        height = payload.height,
        border_color = payload.border_color,
        events = payload.events,
        auto_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
        auto_height = u8::from(payload.auto_max_height),
        max_height = payload.max_height,
        horizontal = u8::from(payload.horizontal_stretch),
        vertical = u8::from(payload.vertical_stretch),
    ))
}

/// The `{1,…}` payload of a `<FormattedDocumentField>`, sixteen members.
///
/// Members 3 and 4 are the stretch pair, and the corpus cannot say which is
/// which: no item of either corpus spells one without the other, so the two
/// always move together and always to the same value. They are written from
/// `<HorizontalStretch>` and `<VerticalStretch>` respectively, and the caller
/// refuses when the two would differ -- which reproduces 75 of 75 and refuses
/// nothing, rather than taking a coin flip the first time a form carries one
/// of the pair alone.
///
/// Members 12 and 15 are 0 because no item spells `<MaxWidth>` or
/// `<MaxHeight>`; by position they are the pair the sibling payloads carry.
pub(crate) struct NativeFormattedDocumentPayload<'a> {
    pub(crate) width: &'a str,
    pub(crate) height: &'a str,
    pub(crate) horizontal_stretch: bool,
    pub(crate) vertical_stretch: bool,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
    pub(crate) font: &'a str,
    pub(crate) events: &'a str,
    pub(crate) auto_max_width: bool,
    pub(crate) auto_max_height: bool,
}

pub(crate) fn format_formatted_document_payload(
    payload: &NativeFormattedDocumentPayload<'_>,
) -> String {
    format!(
        "{{1,{width},{height},{horizontal},{vertical},0,{{3,4,{{0}}}},{back_color},\
         {border_color},{font},{events},{auto_width},0,0,{auto_height},0}}",
        width = payload.width,
        height = payload.height,
        horizontal = u8::from(payload.horizontal_stretch),
        vertical = u8::from(payload.vertical_stretch),
        back_color = payload.back_color,
        border_color = payload.border_color,
        font = payload.font,
        events = payload.events,
        auto_width = u8::from(payload.auto_max_width),
        auto_height = u8::from(payload.auto_max_height),
    )
}

/// The `{5,…}` payload of a `<TextDocumentField>`, sixteen members.
///
/// Member 15 is **not** where `OnChange` goes: all 20 items that spell it
/// store the empty block here, because the field record keeps `OnChange` in
/// its own member. Member 3 is constant 1 over all 159 records because no
/// field of either corpus spells `<HorizontalStretch>`, so the caller refuses
/// one that does rather than guess which of the two readings it takes.
pub(crate) struct NativeTextDocumentPayload<'a> {
    pub(crate) width: &'a str,
    pub(crate) height: &'a str,
    pub(crate) vertical_stretch: bool,
    pub(crate) back_color: &'a str,
    pub(crate) font: &'a str,
    pub(crate) auto_max_width: bool,
    pub(crate) max_width: &'a str,
    pub(crate) auto_max_height: bool,
    pub(crate) max_height: &'a str,
    pub(crate) events: &'a str,
}

pub(crate) fn format_text_document_payload(payload: &NativeTextDocumentPayload<'_>) -> String {
    format!(
        "{{5,{width},{height},1,{vertical},0,{{3,4,{{0}}}},{back_color},{{3,4,{{0}}}},{font},\
         {auto_width},{max_width},0,{auto_height},{max_height},{events}}}",
        width = payload.width,
        height = payload.height,
        vertical = u8::from(payload.vertical_stretch),
        back_color = payload.back_color,
        font = payload.font,
        auto_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
        auto_height = u8::from(payload.auto_max_height),
        max_height = payload.max_height,
        events = payload.events,
    )
}

pub(crate) fn format_popup_payload(payload: &NativePopupPayload<'_>) -> Option<String> {
    let representation = root_code(
        payload.representation,
        &[("Text", "0"), ("Picture", "1"), ("PictureAndText", "2")],
        "3",
    )?;
    let shape = root_code(payload.shape, &[("Usual", "1"), ("Oval", "2")], "0")?;
    let shape_representation = root_code(
        payload.shape_representation,
        &[("Always", "1"), ("WhenActive", "2"), ("None", "3")],
        "0",
    )?;
    Some(format!(
        "{{7,{picture},{command_source},2,{representation},{shape},{shape_representation},{back_color},{border_color}}}",
        picture = payload.picture,
        command_source = payload.command_source,
        back_color = payload.back_color,
        border_color = payload.border_color,
    ))
}

/// The `{2,…}` payload of a `<ColumnGroup>` -- the same wrapper a button group
/// carries, with twelve members instead of four.
///
/// Six carry an XML property: `<Group>` (`Horizontal` 0, `InCell` 2, 1 when
/// the group names none), `<ShowTitle>`, `<ShowInHeader>`,
/// `<HeaderHorizontalAlign>`, the title background and `<FixingInTable>`. All
/// 7 076 of the corpus are reproduced exactly.
pub(crate) struct NativeColumnGroupPayload<'a> {
    pub(crate) group: Option<&'a str>,
    pub(crate) show_title: bool,
    pub(crate) show_in_header: bool,
    pub(crate) header_horizontal_align: Option<&'a str>,
    pub(crate) picture: &'a str,
    pub(crate) title_back_color: &'a str,
    pub(crate) fixing_in_table: Option<&'a str>,
}

impl NativeColumnGroupPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            group: None,
            show_title: true,
            show_in_header: false,
            header_horizontal_align: None,
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            title_back_color: "{3,4,{0}}",
            fixing_in_table: None,
        }
    }
}

pub(crate) fn format_column_group_payload(
    payload: &NativeColumnGroupPayload<'_>,
) -> Option<String> {
    let group = root_code(payload.group, &[("Horizontal", "0"), ("InCell", "2")], "1")?;
    let header_align = root_code(
        payload.header_horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let fixing = root_code(
        payload.fixing_in_table,
        &[("Left", "1"), ("Right", "2")],
        "0",
    )?;
    Some(format!(
        "{{2,{group},{show_title},{show_in_header},{header_align},{picture},{title_back_color},{{0}},{{\"Pattern\"}},\"\",{{1,0}},{fixing}}}",
        show_title = u8::from(payload.show_title),
        show_in_header = u8::from(payload.show_in_header),
        picture = payload.picture,
        title_back_color = payload.title_back_color,
    ))
}

/// What a `{18,…}` page payload carries.
///
/// Fifteen of its twenty members carry an XML property, named by the partition
/// test over all 11 804 pages of the corpus. `<Group>` is written three times:
/// slot 2 says whether the page names one at all, and slots 16 and 17 carry
/// which, differing on `AlwaysHorizontal` and `HorizontalIfPossible`.
pub(crate) struct NativePagePayload<'a> {
    pub(crate) group: Option<&'a str>,
    pub(crate) picture: &'a str,
    pub(crate) child_items_width: Option<&'a str>,
    pub(crate) title_data_path: &'a str,
    pub(crate) format: &'a str,
    pub(crate) show_title: bool,
    pub(crate) back_color: &'a str,
    pub(crate) horizontal_spacing: Option<&'a str>,
    pub(crate) vertical_spacing: Option<&'a str>,
    pub(crate) horizontal_align: Option<&'a str>,
    pub(crate) vertical_align: Option<&'a str>,
    pub(crate) children_align: Option<&'a str>,
    pub(crate) scroll_on_compress: bool,
    pub(crate) border_color: &'a str,
    pub(crate) font: &'a str,
}

impl NativePagePayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            group: None,
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            child_items_width: None,
            title_data_path: "{0}",
            format: "{1,0}",
            show_title: true,
            back_color: "{3,4,{0}}",
            horizontal_spacing: None,
            vertical_spacing: None,
            horizontal_align: None,
            vertical_align: None,
            children_align: None,
            scroll_on_compress: false,
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_page_payload(payload: &NativePagePayload<'_>) -> Option<String> {
    let named = u8::from(payload.group.is_some());
    let horizontal = root_code(
        payload.group,
        &[
            ("Vertical", "0"),
            ("Horizontal", "1"),
            ("AlwaysHorizontal", "1"),
            ("HorizontalIfPossible", "2"),
        ],
        "0",
    )?;
    let horizontal_tail = root_code(
        payload.group,
        &[
            ("Vertical", "0"),
            ("Horizontal", "1"),
            ("AlwaysHorizontal", "3"),
            ("HorizontalIfPossible", "2"),
        ],
        "0",
    )?;
    let child_items_width = root_code(
        payload.child_items_width,
        &[
            ("Equal", "1"),
            ("LeftWide", "2"),
            ("LeftWidest", "3"),
            ("LeftNarrow", "4"),
            ("LeftNarrowest", "5"),
        ],
        "0",
    )?;
    let spacing = |value| {
        root_code(
            value,
            &[
                ("None", "1"),
                ("Half", "2"),
                ("Single", "3"),
                ("OneAndHalf", "4"),
                ("Double", "5"),
            ],
            "0",
        )
    };
    let horizontal_spacing = spacing(payload.horizontal_spacing)?;
    let vertical_spacing = spacing(payload.vertical_spacing)?;
    let horizontal_align = root_code(
        payload.horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let vertical_align = root_code(
        payload.vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    let children_align = root_code(
        payload.children_align,
        &[
            ("None", "1"),
            ("ItemsLeftTitlesLeft", "2"),
            ("ItemsRightTitlesLeft", "3"),
            ("ItemsLeftTitlesRight", "4"),
            ("ItemsRightTitlesRight", "5"),
            ("TitlesLeftDataAuto", "6"),
        ],
        "0",
    )?;
    Some(format!(
        "{{18,{picture},{named},{child_items_width},{title_data_path},{format},{show_title},{{\"Pattern\"}},\"\",{back_color},{horizontal_spacing},{vertical_spacing},{horizontal_align},{vertical_align},{children_align},{scroll_on_compress},{horizontal},{horizontal_tail},{border_color},{font}}}",
        picture = payload.picture,
        title_data_path = payload.title_data_path,
        format = payload.format,
        show_title = u8::from(payload.show_title),
        back_color = payload.back_color,
        scroll_on_compress = u8::from(payload.scroll_on_compress),
        border_color = payload.border_color,
        font = payload.font,
    ))
}

/// What a `{22,…}` group record needs beyond the frame every group shares.
///
/// A container a form can hold, as the body stores it.
///
/// Every container is a `{22,…}` record and one member tells them apart:
/// member 5 is the kind, and the correspondence is exact -- command bar 0,
/// popup 1, column group 2, pages 3, page 4, usual group 5, button group 6,
/// navigator 7, context menu 8, auto command bar 9, with no kind sharing a
/// number.
///
/// The head is **not** fixed-length: member 4 is a flag, and when it is 1 a
/// functional-options block follows before the kind. Lifting that block out is
/// what makes every member past 3 line up; reading the head as fixed leaves
/// 6.5% of the records unaligned.
///
/// Measured over the 9 357 container records of `join-22.tsv`, across all nine
/// kinds: the head rebuilds byte for byte in **all 9 357**, and the tail in
/// 9 354. The three that differ are column groups that both sit in a cell and
/// carry an extended tooltip, and they write 1 where every other column group
/// writes 0.
pub(crate) struct NativeGroupItem<'a> {
    pub(crate) id: &'a str,
    /// Member 5 -- see [`native_group_kind`].
    pub(crate) kind: u8,
    /// The functional-options block, when the group restricts itself.
    pub(crate) functional_options: Option<&'a str>,
    pub(crate) name: &'a str,
    /// Already formatted -- see [`format_russian_title`].
    pub(crate) title: &'a str,
    pub(crate) tooltip_title: &'a str,
    /// `<EnableContentChange>`, `<Enabled>` and `<ReadOnly>`.
    pub(crate) enable_content_change: bool,
    pub(crate) enabled: bool,
    pub(crate) read_only: bool,
    /// `<Width>` and `<Height>`, 0 when the group names neither.
    pub(crate) width: Option<&'a str>,
    pub(crate) height: Option<&'a str>,
    /// `<HorizontalStretch>` and `<VerticalStretch>`: 2 when unnamed.
    pub(crate) horizontal_stretch: Option<bool>,
    pub(crate) vertical_stretch: Option<bool>,
    /// Already formatted -- see [`format_native_color`] and
    /// [`format_native_font`].
    pub(crate) back_color: &'a str,
    pub(crate) font: &'a str,
    pub(crate) payload: &'a str,
    /// `(group uuid, child record)` in the order the body stores them.
    pub(crate) children: &'a [(&'a str, String)],
    /// `<Visible>`, on unless the group turns it off.
    pub(crate) visible: bool,
    /// `<ToolTipRepresentation>`: `None`, `Button`, `ShowTop`, `ShowBottom`
    /// or `Auto`.
    pub(crate) tooltip_representation: Option<&'a str>,
    /// Already formatted -- see [`format_extended_tooltip`].
    pub(crate) extended_tooltip: Option<&'a str>,
    /// `<GroupHorizontalAlign>` and `<GroupVerticalAlign>`.
    pub(crate) horizontal_align: Option<&'a str>,
    pub(crate) vertical_align: Option<&'a str>,
    /// The last member, see [`native_display_importance`].
    pub(crate) display_importance: &'a str,
    /// Member 18, `<Shortcut>`, `{0,0,0}` by default.
    pub(crate) shortcut: &'a str,
}

impl Default for NativeGroupItem<'_> {
    fn default() -> Self {
        Self {
            id: "0",
            kind: 5,
            functional_options: None,
            name: "",
            title: "{1,0}",
            tooltip_title: "{1,0}",
            enable_content_change: false,
            enabled: true,
            read_only: false,
            width: None,
            height: None,
            horizontal_stretch: None,
            vertical_stretch: None,
            back_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            payload: "",
            children: &[],
            visible: true,
            tooltip_representation: None,
            extended_tooltip: None,
            horizontal_align: None,
            vertical_align: None,
            display_importance: "0",
            shortcut: "{0,0,0}",
        }
    }
}

/// Member 5 of a `{22,…}` record, by the element that names the container.
pub(crate) fn native_group_kind(tag: &str) -> Option<u8> {
    Some(match tag {
        "CommandBar" => 0,
        "Popup" => 1,
        "ColumnGroup" => 2,
        "Pages" => 3,
        "Page" => 4,
        "UsualGroup" => 5,
        "ButtonGroup" => 6,
        "ContextMenu" => 8,
        "AutoCommandBar" => 9,
        _ => return None,
    })
}

/// How a stretch reads: 2 when the item names neither value.
fn native_stretch(value: Option<bool>) -> &'static str {
    match value {
        None => "2",
        Some(true) => "1",
        Some(false) => "0",
    }
}

/// The `{22,…}` record of a container, with its children in place.
pub(crate) fn format_group_item(group: &NativeGroupItem<'_>) -> Option<String> {
    let mut children = String::new();
    for (group_uuid, record) in group.children {
        children.push(',');
        children.push_str(group_uuid);
        children.push(',');
        children.push_str(record);
    }
    let options = match group.functional_options {
        Some(block) => format!("1,{block}"),
        None => "0".to_string(),
    };
    let tooltip = match group.extended_tooltip {
        Some(record) => format!("1,{record}"),
        None => "0".to_string(),
    };
    let tooltip_representation = root_code(
        group.tooltip_representation,
        &[
            ("Auto", "0"),
            ("None", "1"),
            ("Balloon", "2"),
            ("Button", "3"),
            ("ShowAuto", "4"),
            ("ShowTop", "5"),
            ("ShowLeft", "6"),
            ("ShowBottom", "7"),
            ("ShowRight", "8"),
        ],
        "0",
    )?;
    let horizontal = root_code(
        group.horizontal_align,
        &[("Left", "0"), ("Center", "1"), ("Right", "2")],
        "3",
    )?;
    let vertical = root_code(
        group.vertical_align,
        &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
        "3",
    )?;
    Some(format!(
        "{{22,{{{id},{ns}}},0,0,{options},{kind},{name},{title},{tooltip_title},\
         {content_change},{enabled},{read_only},{width},{height},{horizontal_stretch},\
         {vertical_stretch},{back_color},{font},{shortcut},1,{payload},{count}{children},\
         {visible},{tooltip_representation},{tooltip},0,{horizontal},{vertical},\
         {display_importance}}}",
        id = group.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = group.kind,
        name = quoted(group.name),
        title = group.title,
        tooltip_title = group.tooltip_title,
        content_change = u8::from(group.enable_content_change),
        enabled = u8::from(group.enabled),
        read_only = u8::from(group.read_only),
        width = group.width.unwrap_or("0"),
        height = group.height.unwrap_or("0"),
        horizontal_stretch = native_stretch(group.horizontal_stretch),
        vertical_stretch = native_stretch(group.vertical_stretch),
        back_color = group.back_color,
        font = group.font,
        payload = group.payload,
        count = group.children.len(),
        visible = u8::from(group.visible),
        display_importance = group.display_importance,
        shortcut = group.shortcut,
    ))
}

/// What a form says about itself before its property bag -- the root record's
/// head, the first 18 members of the `{50,…}` record.
///
/// Measured over the 12 469 ERP УХ forms that do not name a
/// `<SettingsStorage>`: all 12 469 heads rebuild byte for byte from the source
/// alone. A form that names one is refused, because member 8 is then that
/// storage object's uuid, which only the configuration can resolve.
pub(crate) struct NativeRootHead<'a> {
    /// `<WindowOpeningMode>`: `LockOwnerWindow`, `LockWholeInterface` or
    /// `DontUse`.
    pub(crate) window_opening_mode: Option<&'a str>,
    /// `<Width>` and `<Height>`, in characters.
    pub(crate) width: Option<&'a str>,
    pub(crate) height: Option<&'a str>,
    /// `<EnterKeyBehavior>`, of which only `DefaultButton` is ever stored.
    pub(crate) enter_key_behavior: Option<&'a str>,
    /// `<SaveDataInSettings>`, of which only `UseList` is ever stored.
    pub(crate) save_data_in_settings: Option<&'a str>,
    /// `<AutoSaveDataInSettings>`, of which only `Use` is ever stored.
    pub(crate) auto_save_data_in_settings: Option<&'a str>,
    /// The uuid of the `<SettingsStorage>` the form names.
    pub(crate) settings_storage: Option<&'a str>,
    /// `<AutoTitle>`, on unless the form turns it off.
    pub(crate) auto_title: bool,
    /// Already formatted -- see [`format_russian_title`]. A form that names no
    /// title writes `{1,0}`.
    pub(crate) title: &'a str,
    /// `<Group>`: member 11 only says whether the form names one at all.
    pub(crate) group: Option<&'a str>,
    /// `<ChildItemsWidth>`: `Equal`, `LeftWide`, `LeftWidest`, `LeftNarrow` or
    /// `LeftNarrowest`.
    pub(crate) child_items_width: Option<&'a str>,
    /// `<AutoFillCheck>`, `<Customizable>` and `<Enabled>`, all on unless the
    /// form turns them off.
    pub(crate) auto_fill_check: bool,
    pub(crate) customizable: bool,
    pub(crate) enabled: bool,
    /// `<CommandBarLocation>`: `None`, `Top`, `Bottom` or `Auto`.
    pub(crate) command_bar_location: Option<&'a str>,
}

impl Default for NativeRootHead<'_> {
    fn default() -> Self {
        Self {
            window_opening_mode: None,
            width: None,
            height: None,
            enter_key_behavior: None,
            save_data_in_settings: None,
            auto_save_data_in_settings: None,
            settings_storage: None,
            auto_title: true,
            title: "{1,0}",
            group: None,
            child_items_width: None,
            auto_fill_check: true,
            customizable: true,
            enabled: true,
            command_bar_location: None,
        }
    }
}

/// The root record's head, from `50` to the command bar's location.
pub(crate) fn format_root_head(head: &NativeRootHead<'_>) -> Option<String> {
    let members = [
        "50".to_string(),
        "0".to_string(),
        root_code(
            head.window_opening_mode,
            &[
                ("DontUse", "0"),
                ("LockOwnerWindow", "1"),
                ("LockWholeInterface", "2"),
            ],
            "0",
        )?,
        head.width.unwrap_or("0").to_string(),
        head.height.unwrap_or("0").to_string(),
        root_code(head.enter_key_behavior, &[("DefaultButton", "0")], "1")?,
        root_code(head.save_data_in_settings, &[("UseList", "1")], "0")?,
        root_code(head.auto_save_data_in_settings, &[("Use", "1")], "0")?,
        // Member 8: the uuid of the `<SettingsStorage>` the form names, which
        // the caller resolves against the configuration.
        head.settings_storage
            .unwrap_or("00000000-0000-0000-0000-000000000000")
            .to_string(),
        u8::from(head.auto_title).to_string(),
        head.title.to_string(),
        u8::from(head.group.is_some()).to_string(),
        root_code(
            head.child_items_width,
            &[
                ("Equal", "1"),
                ("LeftWide", "2"),
                ("LeftWidest", "3"),
                ("LeftNarrow", "4"),
                ("LeftNarrowest", "5"),
            ],
            "0",
        )?,
        u8::from(head.auto_fill_check).to_string(),
        u8::from(head.customizable).to_string(),
        u8::from(head.enabled).to_string(),
        "0".to_string(),
        root_code(
            head.command_bar_location,
            &[("None", "0"), ("Auto", "1"), ("Top", "2"), ("Bottom", "3")],
            "1",
        )?,
    ];
    Some(members.join(","))
}

/// What a form says about itself after its children -- the root record's tail.
///
/// The tail opens with two empty strings and an optional navigator group, and
/// what follows is exactly 21 members: the form's own property bag. Measured
/// over the 12 410 ERP УХ forms that do not carry a
/// `<MobileDeviceCommandBarContent>`, all 12 410 tails rebuild byte for byte
/// from the source alone.
///
/// That mobile command bar reaches the tail as `{50,1,"",{"N",<n>}}`, and the
/// `<n>` is the id of the form **item** its `<xr:Value>` names -- so the
/// source does resolve it. See [`format_mobile_device_command_bar_content`],
/// which the caller fills this field from.
pub(crate) struct NativeRootTail<'a> {
    /// `<AutoURL>`, which is on unless the form turns it off.
    pub(crate) auto_url: bool,
    /// `<VerticalScroll>`: `useIfNecessary` or `useWithoutStretch`. It is read
    /// twice, and not the same way both times -- `useWithoutStretch` writes 0
    /// in the first place and 3 in the second.
    pub(crate) vertical_scroll: Option<&'a str>,
    /// `<ScalingMode>`: `Normal` or `Compact`.
    pub(crate) scaling_mode: Option<&'a str>,
    /// `<HorizontalSpacing>` and `<VerticalSpacing>`: `None`, `Half`,
    /// `OneAndHalf` or `Double`.
    pub(crate) horizontal_spacing: Option<&'a str>,
    pub(crate) vertical_spacing: Option<&'a str>,
    /// `<HorizontalAlign>`: `Left`, `Center` or `Right`.
    pub(crate) horizontal_align: Option<&'a str>,
    /// `<VerticalAlign>`: `Top`, `Center` or `Bottom`.
    pub(crate) vertical_align: Option<&'a str>,
    /// `<ChildrenAlign>`, of which only `None` is ever stored.
    pub(crate) children_align: Option<&'a str>,
    /// `<Group>`, read twice and again not the same way: `AlwaysHorizontal`
    /// writes 1 in the first place and 3 in the second.
    pub(crate) group: Option<&'a str>,
    /// `<ShowTitle>` and `<ShowCloseButton>`, both on unless turned off.
    pub(crate) show_title: bool,
    pub(crate) show_close_button: bool,
    /// `<ConversationsRepresentation>`: `Show` or `DontShow`.
    pub(crate) conversations_representation: Option<&'a str>,
    /// `<CollapseItemsByImportanceVariant>`: `Use` or `DontUse`.
    pub(crate) collapse_items_by_importance: Option<&'a str>,
    /// `<SaveWindowSettings>`, on unless turned off.
    pub(crate) save_window_settings: bool,
    /// The navigator group's record, when the form has one.
    pub(crate) navigator: Option<&'a str>,
    /// `<MobileDeviceCommandBarContent>`, already formatted by
    /// [`format_mobile_device_command_bar_content`] -- `{50,0}` for the
    /// 13 443 forms of both corpora that do not carry the element.
    pub(crate) mobile_device_command_bar_content: &'a str,
}

impl Default for NativeRootTail<'_> {
    fn default() -> Self {
        Self {
            auto_url: true,
            vertical_scroll: None,
            scaling_mode: None,
            horizontal_spacing: None,
            vertical_spacing: None,
            horizontal_align: None,
            vertical_align: None,
            children_align: None,
            group: None,
            show_title: true,
            show_close_button: true,
            conversations_representation: None,
            collapse_items_by_importance: None,
            save_window_settings: true,
            navigator: None,
            mobile_device_command_bar_content: "{50,0}",
        }
    }
}

/// `<MobileDeviceCommandBarContent>`, in the shape trailer slot `22 + blocks`
/// stores it.
///
/// ```text
/// {<root>, N, <presentation>, <value>, <presentation>, <value>, …}
/// ```
///
/// `2 + 2N` members, one pair per `<xr:Item>` in XML order: the item's
/// `<xr:Presentation>`, which is `""` in all 214 items of both corpora, and
/// its `<xr:Value>` as `{"N",<the named item's id>}`, or `{"N",0}` when the
/// element is empty.
///
/// The partition is pure over every form of both corpora with a readable root
/// trailer (12 500 ERP УХ + 1 103 BSP): the element absent stores `{50,0}` in
/// 13 443 of 13 443, and present stores `{50,N,…}` with N the `<xr:Item>`
/// count in 160 of 160.
///
/// The caller resolves the name, over the form's items and never its
/// `<Command>`s -- in all ten forms where an item and a command share a name
/// the item's id is what the body holds.
pub(crate) fn format_mobile_device_command_bar_content(item_ids: &[&str]) -> String {
    if item_ids.is_empty() {
        return "{50,0}".to_string();
    }
    let mut out = format!("{{50,{}", item_ids.len());
    for id in item_ids {
        out.push_str(",\"\",{\"N\",");
        out.push_str(id);
        out.push('}');
    }
    out.push('}');
    out
}

/// Reads one spelling of a property, refusing any the corpus never showed.
fn root_code(value: Option<&str>, table: &[(&str, &str)], absent: &'static str) -> Option<String> {
    match value {
        None => Some(absent.to_string()),
        Some(value) => table
            .iter()
            .find(|(candidate, _)| *candidate == value)
            .map(|(_, code)| (*code).to_string()),
    }
}

const ROOT_SPACING: &[(&str, &str)] = &[
    ("None", "1"),
    ("Half", "2"),
    ("OneAndHalf", "4"),
    ("Double", "5"),
];

/// The root record's tail, from `"",""` to the last member of the record.
pub(crate) fn format_root_tail(tail: &NativeRootTail<'_>) -> Option<String> {
    let scroll = root_code(
        tail.vertical_scroll,
        &[("useIfNecessary", "2"), ("useWithoutStretch", "0")],
        "0",
    )?;
    let scroll_again = root_code(
        tail.vertical_scroll,
        &[("useIfNecessary", "2"), ("useWithoutStretch", "3")],
        "0",
    )?;
    let group = root_code(
        tail.group,
        &[
            ("Vertical", "0"),
            ("Horizontal", "1"),
            ("AlwaysHorizontal", "1"),
            ("HorizontalIfPossible", "2"),
        ],
        "0",
    )?;
    let group_again = root_code(
        tail.group,
        &[
            ("Vertical", "0"),
            ("Horizontal", "1"),
            ("AlwaysHorizontal", "3"),
            ("HorizontalIfPossible", "2"),
        ],
        "0",
    )?;
    let members = [
        u8::from(tail.auto_url).to_string(),
        "\"\"".to_string(),
        scroll,
        root_code(tail.scaling_mode, &[("Normal", "1"), ("Compact", "2")], "0")?,
        "0".to_string(),
        "0".to_string(),
        root_code(tail.horizontal_spacing, ROOT_SPACING, "0")?,
        root_code(tail.vertical_spacing, ROOT_SPACING, "0")?,
        root_code(
            tail.horizontal_align,
            &[("Left", "0"), ("Center", "1"), ("Right", "2")],
            "3",
        )?,
        root_code(
            tail.vertical_align,
            &[("Top", "0"), ("Center", "1"), ("Bottom", "2")],
            "3",
        )?,
        root_code(tail.children_align, &[("None", "1")], "0")?,
        group,
        scroll_again,
        "100".to_string(),
        u8::from(tail.show_title).to_string(),
        u8::from(tail.show_close_button).to_string(),
        root_code(
            tail.conversations_representation,
            &[("Show", "1"), ("DontShow", "2")],
            "0",
        )?,
        root_code(
            tail.collapse_items_by_importance,
            &[("Use", "1"), ("DontUse", "2")],
            "0",
        )?,
        group_again,
        tail.mobile_device_command_bar_content.to_string(),
        u8::from(tail.save_window_settings).to_string(),
    ];
    let head = match tail.navigator {
        Some(navigator) => format!("\"\",\"\",1,{navigator}"),
        None => "\"\",\"\",0".to_string(),
    };
    Some(format!("{head},{}", members.join(",")))
}

pub(crate) struct NativeRootLayout<'a> {
    /// The first 18 members, from [`format_root_head`].
    pub(crate) head: &'a str,
    /// The keyed property bag: a count, then that many `(key, value)` pairs.
    /// Its shape holds in all 12 488 records of the corpus that split, and
    /// exactly four members follow it -- the events, the command set, the
    /// command bar's flag and the bar.
    pub(crate) properties: &'a [(&'a str, String)],
    /// The form's own event bindings, or `{0,1,0}` when it has none.
    pub(crate) events: &'a str,
    /// The root `<CommandSet>`, `{0}` when the form excludes nothing.
    pub(crate) command_set: &'a str,
    /// The auto command bar's record.
    pub(crate) command_bar: &'a str,
    /// The form's own child items, as `(kind uuid, record)` -- the same
    /// encoding a group uses, with the count first. See [`child_kind_uuid`].
    pub(crate) children: &'a [(&'a str, String)],
    /// Everything after the children, from [`format_root_tail`].
    pub(crate) tail: &'a str,
}

/// The `{50,…}` root layout of a form body.
pub(crate) fn format_root_layout(root: &NativeRootLayout<'_>) -> String {
    let mut children = String::new();
    for (kind_uuid, record) in root.children {
        children.push(',');
        children.push_str(kind_uuid);
        children.push(',');
        children.push_str(record);
    }
    let mut properties = String::new();
    for (key, value) in root.properties {
        properties.push(',');
        properties.push_str(key);
        properties.push(',');
        properties.push_str(value);
    }
    format!(
        "{{{head},{bag}{properties},{events},{command_set},1,{command_bar},\
         {count}{children},{tail}}}",
        head = root.head,
        bag = root.properties.len(),
        events = root.events,
        command_set = root.command_set,
        command_bar = root.command_bar,
        count = root.children.len(),
        tail = root.tail,
    )
}

/// The `{5,…}` record of a table's search string, view status or search
/// control addition.
///
/// `kind` is `0` for the search string, `1` for the view status and `2` for
/// the search control, and the record closes by naming the table it belongs to
/// as `{<table id>,<kind>}`. The payload differs per kind -- the view status
/// carries five appearance blocks where the search string carries three -- so
/// it arrives already formatted, like every other slot whose XML property is
/// not yet named.
pub(crate) fn format_search_addition(
    id: &str,
    kind: u8,
    name: &str,
    payload: &str,
    context_menu: &str,
    extended_tooltip: &str,
    table_id: &str,
) -> String {
    format!(
        "{{5,{{{id},{ns}}},0,0,0,{kind},{name},{{1,0}},{{1,0}},1,1,0,1,{payload},1,\
         {context_menu},1,{extended_tooltip},2,{{{table_id},{kind}}},0,3,3,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
    )
}

/// What a `{55,…}` table record carries around its six nested children.
///
/// The head grows with what the table shows, but the tail does not. Measured
/// over the `{55,…}` records of the first 800 ERP УХ form bodies: the command
/// bar sits exactly two members after the context menu in all 344 of them, the
/// extended tooltip exactly 26 members from the end in 342, and the first
/// `{5,…}` addition exactly 21 from the end in 335. The three additions follow
/// each other two members apart and sixteen scalars close the record.
///
/// So the parts a caller supplies are the head and the scalar runs; this
/// writer owns the structure between them.
pub(crate) struct NativeTableItem<'a> {
    /// Everything from `55` up to the member before the context menu's flag,
    /// already joined with commas.
    pub(crate) head: &'a str,
    pub(crate) context_menu: &'a str,
    pub(crate) command_bar: &'a str,
    /// The twelve members between the command bar and the tooltip's flag.
    pub(crate) middle: &'a str,
    pub(crate) extended_tooltip: &'a str,
    /// Where the table says what it shows -- the search string, the view
    /// status and the search control -- which is exactly the three members
    /// between the tooltip and the first addition's flag. Measured over all
    /// 7 183 table records of the ERP УХ corpus: a candidate filling these
    /// three, with `<Representation>` and `<SkipOnInput>` which live in the
    /// head and the middle, reproduces every one of them exactly.
    pub(crate) search_string_location: Option<&'a str>,
    pub(crate) view_status_location: Option<&'a str>,
    pub(crate) search_control_location: Option<&'a str>,
    pub(crate) search_string_addition: &'a str,
    pub(crate) view_status_addition: &'a str,
    pub(crate) search_control_addition: &'a str,
    /// The sixteen members that close the record.
    pub(crate) tail: &'a str,
}

/// The codes a table writes for where it shows its search string.
const TABLE_SEARCH_STRING_CODES: &[(&str, &str)] = &[
    ("None", "1"),
    ("CommandBar", "2"),
    ("Top", "3"),
    ("Bottom", "4"),
    ("FormCaption", "5"),
    ("PullFromTop", "6"),
];

/// The codes for where it shows its view status.
const TABLE_VIEW_STATUS_CODES: &[(&str, &str)] = &[("None", "1"), ("Top", "2"), ("Bottom", "3")];

/// The codes for where it shows its search control.
const TABLE_SEARCH_CONTROL_CODES: &[(&str, &str)] = &[("None", "1"), ("CommandBar", "2")];

/// The code a table writes for one of those three; a table that names none
/// writes `0`.
fn table_location_code(
    location: Option<&str>,
    codes: &[(&'static str, &'static str)],
) -> &'static str {
    let Some(location) = location else {
        return "0";
    };
    codes
        .iter()
        .find_map(|(name, code)| (*name == location).then_some(*code))
        .unwrap_or("0")
}

/// The `{55,…}` record of a table, with its six children in place.
pub(crate) fn format_table_item(table: &NativeTableItem<'_>) -> String {
    format!(
        "{{{head},1,{context_menu},1,{command_bar},{middle},1,{tooltip},{search_string_location},{view_status_location},{search_control_location},\
         1,{search_string},1,{view_status},1,{search_control},{tail}}}",
        head = table.head,
        context_menu = table.context_menu,
        command_bar = table.command_bar,
        middle = table.middle,
        tooltip = table.extended_tooltip,
        search_string_location =
            table_location_code(table.search_string_location, TABLE_SEARCH_STRING_CODES),
        view_status_location =
            table_location_code(table.view_status_location, TABLE_VIEW_STATUS_CODES),
        search_control_location =
            table_location_code(table.search_control_location, TABLE_SEARCH_CONTROL_CODES),
        search_string = table.search_string_addition,
        view_status = table.view_status_addition,
        search_control = table.search_control_addition,
        tail = table.tail,
    )
}

/// The `{22,…}` record of an empty `<AutoCommandBar>`.
///
/// The one slot that separates it from a context menu is the marker `9`, and
/// the tail carries `{0,0,1}` where a context menu carries `{1,1}`.
pub(crate) fn format_empty_auto_command_bar(id: &str, name: &str) -> String {
    format!(
        "{{22,{{{id},{ns}}},0,0,0,9,{name},{{1,0}},{{1,0}},0,1,0,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{0,0,1}},0,1,0,0,0,3,3,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
    )
}

// ---------------------------------------------------------------------------
// `<DataPath>` -- member 11 of a `{37,…}` field record, and the same member of
// a table, a group title and a page title.
// ---------------------------------------------------------------------------
//
// A data path is walked left to right, carrying the type the previous segment
// resolved to; every segment is looked up in that context. The rule is the one
// measured in `lab/findings/data-path-resolution.md`, which places 135 232 of
// the 135 393 stored records of ERP УХ (99.88 %) and 265 388 of the 265 554
// stored segments, computed forwards from the form and the configuration with
// no reference to the stored value.
//
// Fail-closed throughout: a part the rule cannot name returns `None`, and the
// caller refuses the form rather than writing a binding that would load wrong.

/// The marker a `[<i>]` subscript is stored under.
///
/// 1 932 uses across 56 forms, the first member always the literal index, so
/// `[0]` looks exactly like `{0,<uuid>}` and only the uuid tells them apart.
const DATA_PATH_INDEX_UUID: &str = "e67e2953-cebe-4d97-bb93-12b17e6384f8";

/// The marker a column of a `<Columns><AdditionalColumns table="…">` block
/// carries; 3 835 segments of the corpus need it.
const DATA_PATH_ADDITIONAL_COLUMN_UUID: &str = "5bdad865-f2c5-434b-8041-ba4aad3b6687";

/// `<Object>.SettingsComposer` -- a platform uuid, not a configuration one.
const DATA_PATH_SETTINGS_COMPOSER_UUID: &str = "b9754f01-29e9-11d6-a3c7-0050bae0a776";

/// `Total<Field>` on a tabular section, `RowsCount` on one, and a dynamic
/// list's `DefaultPicture`.
const DATA_PATH_TOTAL_MARKER: &str = "101000000";
const DATA_PATH_ROWS_COUNT_MARKER: &str = "100000000";
const DATA_PATH_DEFAULT_PICTURE_MARKER: &str = "10000000";

/// The members of the builtin (non-configuration) attribute types. Each is a
/// fixed small table; every name maps to one number over every record.
const DATA_PATH_VALUE_LIST_MEMBERS: &[(&str, &str)] = &[
    ("ValueType", "-1"),
    ("Value", "0"),
    ("Presentation", "1"),
    ("Check", "2"),
    ("Picture", "3"),
    ("RowsCount", "100000000"),
];
const DATA_PATH_STANDARD_PERIOD_MEMBERS: &[(&str, &str)] =
    &[("Variant", "0"), ("StartDate", "1"), ("EndDate", "2")];
const DATA_PATH_STANDARD_BEGINNING_DATE_MEMBERS: &[(&str, &str)] =
    &[("Variant", "0"), ("Date", "1")];
const DATA_PATH_COMPOSER_MEMBERS: &[(&str, &str)] =
    &[("Settings", "0"), ("UserSettings", "1"), ("FixedSettings", "2")];
/// A bare `cfg:ReportObject` -- a common form's `Отчет` -- has one member the
/// walk reaches, the composer, stored under the platform's own uuid.
const DATA_PATH_REPORT_OBJECT_MEMBERS: &[(&str, &str)] =
    &[("SettingsComposer", "0,b9754f01-29e9-11d6-a3c7-0050bae0a776")];
const DATA_PATH_GANTT_CHART_MEMBERS: &[(&str, &str)] = &[("Point", "0"), ("Text", "1")];

/// A dynamic list's own members, ahead of its query fields.
const DATA_PATH_DYNAMIC_LIST_MEMBERS: &[(&str, &str)] =
    &[("Order", "-1"), ("Filter", "-2"), ("SettingsComposer", "-6")];

/// The settings-composer sub-tree. The numbering is **per parent collection**:
/// `Presentation` is 10010 under a filter item and 10004 under a
/// conditional-appearance item, and splitting by parent removes every
/// ambiguity.
const DATA_PATH_DCS_SETTINGS: &[(&str, &str)] = &[
    ("DataParameters", "0"),
    ("Filter", "1"),
    ("Selection", "2"),
    ("Order", "3"),
    ("ConditionalAppearance", "4"),
    ("OutputParameters", "5"),
    ("UserFields", "6"),
    ("Use", "10000"),
    ("ReportStructure", "10001"),
    ("HasSelection", "10002"),
    ("HasFilter", "10003"),
    ("HasOrder", "10004"),
    ("HasConditionalAppearance", "10005"),
    ("HasOutputParameters", "10006"),
    ("ItemDataParameters", "10007"),
    ("ItemFilter", "10008"),
    ("ItemGroupFields", "10009"),
    ("ItemSelection", "10010"),
    ("ItemOrder", "10011"),
    ("ItemConditionalAppearance", "10012"),
    ("ItemOutputParameters", "10013"),
    ("ItemUserFields", "10014"),
    ("ReportStructurePicture", "10015"),
];
const DATA_PATH_DCS_SELECTION: &[(&str, &str)] = &[("SelectionAvailableFields", "0")];
const DATA_PATH_DCS_GROUP_FIELDS: &[(&str, &str)] = &[("GroupFieldsAvailableFields", "0")];
const DATA_PATH_DCS_FILTER: &[(&str, &str)] = &[
    ("FilterAvailableFields", "0"),
    ("Use", "10000"),
    ("LeftValuePicture", "10001"),
    ("LeftValue", "10002"),
    ("ComparisonType", "10003"),
    ("RightValuePicture", "10004"),
    ("RightValue", "10005"),
    ("Date", "10006"),
    ("GroupType", "10007"),
    ("Application", "10008"),
    ("ViewMode", "10009"),
    ("Presentation", "10010"),
];
const DATA_PATH_DCS_CONDITIONAL_APPEARANCE: &[(&str, &str)] = &[
    ("Use", "10000"),
    ("Appearance", "10001"),
    ("Filter", "10002"),
    ("Fields", "10003"),
    ("Presentation", "10004"),
    ("UseArea", "10005"),
];
const DATA_PATH_DCS_ORDER: &[(&str, &str)] = &[
    ("OrderAvailableFields", "0"),
    ("Use", "10000"),
    ("Field", "10002"),
    ("OrderType", "10003"),
];
const DATA_PATH_DCS_USER_SETTINGS: &[(&str, &str)] = &[
    ("Use", "10000"),
    ("SettingPicture", "10001"),
    ("Setting", "10002"),
    ("ComparisonType", "10003"),
    ("ValuePicture", "10004"),
    ("Value", "10005"),
    ("EditInReportForm", "10006"),
    ("Filter", "10007"),
    ("Order", "10008"),
    ("ConditionalAppearance", "10010"),
    ("Structure", "10011"),
];
const DATA_PATH_DCS_APPEARANCE: &[(&str, &str)] = &[
    ("Use", "10000"),
    ("Parameter", "10001"),
    ("ValuePicture", "10002"),
    ("Value", "10003"),
];
const DATA_PATH_DCS_FILTER_AVAILABLE_FIELDS: &[(&str, &str)] =
    &[("FieldPicture", "10000"), ("Title", "10001")];
const DATA_PATH_DCS_FIELDS: &[(&str, &str)] =
    &[("Use", "10000"), ("FieldPicture", "10001"), ("Field", "10002")];

/// The standard-attribute table: `(scope, name)` to the negative number the
/// body stores.
///
/// Measured over the corpus; each pair maps to exactly one number over every
/// record that uses it, and **0 pairs take more than one value**. The number is
/// not an ordinal into the object's `<StandardAttributes>` and is not derivable
/// from the source tree -- it is a platform constant, and it differs per class
/// (`Description` is −3 on a catalog, −8 on a chart of accounts, −9 on a chart
/// of characteristic types and on a task).
///
/// This is the table the corpus exercises, not the platform's whole table:
/// names this configuration never binds -- `DeletionMark`, `IsFolder`,
/// `Presentation`, `DataVersion` and more -- are simply absent, and a name the
/// table does not know refuses rather than taking a default. 17 of the 64 pairs
/// rest on a single record; the other 47 are each confirmed 2 to 901 times.
const DATA_PATH_STANDARD_ATTRIBUTES: &[(&str, &str, &str)] = &[
    ("<TabularSection>", "LineNumber", "-2"),
    ("AccountingRegister", "Period", "-2"),
    ("AccountingRegister", "Recorder", "-3"),
    ("AccountingRegister", "LineNumber", "-4"),
    ("AccountingRegister", "Active", "-5"),
    ("AccountingRegister", "AccountDr", "-6"),
    ("AccountingRegister", "AccountCr", "-7"),
    ("AccountingRegister", "Account", "-10"),
    ("AccountingRegister", "PeriodAdjustment", "-30"),
    ("AccountingRegister", "Filter", "-60001"),
    ("AccumulationRegister", "Period", "-2"),
    ("AccumulationRegister", "LineNumber", "-4"),
    ("AccumulationRegister", "RecordType", "-9"),
    ("BusinessProcess", "Number", "-2"),
    ("BusinessProcess", "Date", "-3"),
    ("Catalog", "Code", "-2"),
    ("Catalog", "Description", "-3"),
    ("Catalog", "Parent", "-4"),
    ("Catalog", "Owner", "-5"),
    ("Catalog", "Ref", "-8"),
    ("Catalog", "Predefined", "-10"),
    ("Catalog", "PredefinedDataName", "-13"),
    ("ChartOfAccounts", "Parent", "-6"),
    ("ChartOfAccounts", "Code", "-7"),
    ("ChartOfAccounts", "Description", "-8"),
    ("ChartOfAccounts", "Type", "-10"),
    ("ChartOfAccounts", "OffBalance", "-11"),
    ("ChartOfAccounts", "ExtDimensionTypes", "-12"),
    ("ChartOfAccounts", "Order", "-17"),
    ("ChartOfAccounts", "Ref", "-2"),
    ("ChartOfAccounts/ExtDimensionTypes", "ExtDimensionType", "-13"),
    ("ChartOfAccounts/ExtDimensionTypes", "TurnoversOnly", "-15"),
    ("ChartOfCalculationTypes", "Code", "-2"),
    ("ChartOfCalculationTypes", "Description", "-3"),
    ("ChartOfCalculationTypes", "ActionPeriodIsBasic", "-4"),
    ("ChartOfCalculationTypes", "BaseCalculationTypes", "-10"),
    ("ChartOfCalculationTypes", "DisplacingCalculationTypes", "-20"),
    ("ChartOfCalculationTypes", "LeadingCalculationTypes", "-30"),
    (
        "ChartOfCalculationTypes/BaseCalculationTypes",
        "CalculationType",
        "-101",
    ),
    (
        "ChartOfCalculationTypes/DisplacingCalculationTypes",
        "CalculationType",
        "-101",
    ),
    (
        "ChartOfCalculationTypes/LeadingCalculationTypes",
        "CalculationType",
        "-101",
    ),
    ("ChartOfCharacteristicTypes", "Parent", "-6"),
    ("ChartOfCharacteristicTypes", "Code", "-8"),
    ("ChartOfCharacteristicTypes", "Description", "-9"),
    ("ChartOfCharacteristicTypes", "ValueType", "-11"),
    ("ChartOfCharacteristicTypes", "PredefinedDataName", "-14"),
    ("Document", "Number", "-2"),
    ("Document", "Date", "-3"),
    ("Document", "DeletionMark", "-4"),
    ("Document", "Ref", "-5"),
    ("Document", "Posted", "-7"),
    ("Document", "RegisterRecords", "-8"),
    ("ExchangePlan", "Code", "-2"),
    ("ExchangePlan", "Description", "-3"),
    ("ExchangePlan", "SentNo", "-9"),
    ("ExchangePlan", "ReceivedNo", "-10"),
    ("ExchangePlan", "ThisNode", "-13"),
    ("ExchangePlan", "ExchangeDate", "-14"),
    ("ExchangePlan", "Ref", "-6"),
    ("InformationRegister", "Period", "-2"),
    ("InformationRegister", "Recorder", "-3"),
    ("InformationRegister", "LineNumber", "-4"),
    ("Task", "Number", "-2"),
    ("Task", "Date", "-3"),
    ("Task", "BusinessProcess", "-7"),
    ("Task", "Ref", "-5"),
    ("Task", "RoutePoint", "-8"),
    ("Task", "Description", "-9"),
    ("Task", "Executed", "-10"),
];

/// The standard attributes that do not end the walk, and where they lead.
const DATA_PATH_STANDARD_SECTIONS: [&str; 4] = [
    "ExtDimensionTypes",
    "BaseCalculationTypes",
    "LeadingCalculationTypes",
    "DisplacingCalculationTypes",
];

/// The families a `<Object>.RegisterRecords.<name>` part can name.
const DATA_PATH_REGISTER_FAMILIES: [&str; 4] = [
    "AccountingRegister",
    "AccumulationRegister",
    "InformationRegister",
    "CalculationRegister",
];

/// One field of a metadata object, as the configuration source declares it.
///
/// `uuid` is the field's own `uuid=` attribute, verbatim: a field id is never
/// an ordinal and never a number the object's XML carries anywhere else.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct ConfigurationField {
    pub(crate) uuid: String,
    /// The XML tag the field is declared with -- `Attribute`, `Dimension`,
    /// `TabularSection`, `AccountingFlag` and the rest. Part of the key: all
    /// three charts of accounts name an `<AccountingFlag>` and an
    /// `<ExtDimensionAccountingFlag>` alike.
    pub(crate) tag: String,
    /// The `<Properties><Type>` entries, which say what the next segment
    /// resolves against.
    pub(crate) types: Vec<String>,
}

/// One metadata object of the configuration source tree: its own uuid and the
/// fields it declares, keyed by name.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct ConfigurationObject {
    /// `<MetaDataObject>/<Class uuid=>`.
    pub(crate) uuid: String,
    /// The singular class name, `Catalog`, `Document`, `AccountingRegister`…
    pub(crate) class: String,
    /// `<Properties><Owners>`, for the `Owner` standard attribute.
    pub(crate) owners: Vec<String>,
    /// Every field, keyed by name; a name that two tags share keeps both.
    pub(crate) fields: BTreeMap<String, Vec<ConfigurationField>>,
    /// Each tabular section's own fields.
    pub(crate) sections: BTreeMap<String, BTreeMap<String, Vec<ConfigurationField>>>,
    /// A defined type's own `<Properties><Type>`, which a data path walks on
    /// in as if the attribute had declared it.
    pub(crate) types: Vec<String>,
}

/// The configuration source tree, as the data-path walk asks it questions.
///
/// One method: give me the object `"Catalog.Валюты"`, `"CommonAttribute.X"` or
/// `"Constant.X"`. The implementation reads it from the tree and memoises it,
/// so an object is parsed once per run however many paths name it.
pub(crate) trait ConfigurationObjects {
    fn object(&self, key: &str) -> Option<Arc<ConfigurationObject>>;
}

/// A `<Columns><Column>` or one column of an `<AdditionalColumns>` block.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct DataPathColumn {
    pub(crate) id: String,
    pub(crate) types: Vec<String>,
}

/// A form `<Attribute>`, with the two column tables a data path walks into.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct DataPathAttribute {
    /// A dynamic list's synthetic field map: the id each dotted prefix of a
    /// query field stores, and the ids of the `~`-marked references by prefix
    /// and twin (`findings/rt-dynamic-list.md` §3).
    pub(crate) dynamic_fields: BTreeMap<String, String>,
    pub(crate) dynamic_marked: BTreeMap<(String, Option<String>), String>,
    /// The attribute's own name, which a marked reference's twin repeats.
    pub(crate) list_name: Option<String>,
    pub(crate) id: String,
    pub(crate) types: Vec<String>,
    /// `<Columns><Column name= id=>`.
    pub(crate) columns: BTreeMap<String, DataPathColumn>,
    /// `<Columns><AdditionalColumns table="…">`, keyed by that `table=` path --
    /// a dotted path with no subscripts, rooted at the attribute's own name.
    pub(crate) additional_columns: BTreeMap<String, BTreeMap<String, DataPathColumn>>,
}

/// A form item, for an `Items.<item>` head.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct DataPathItem {
    pub(crate) id: String,
    pub(crate) data_path: Option<String>,
}

/// Everything a data path needs from the form's own `Form.xml`.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct DataPathForm {
    pub(crate) attributes: BTreeMap<String, DataPathAttribute>,
    pub(crate) items: BTreeMap<String, DataPathItem>,
}

/// The context the walk carries: the type the previous segment resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DataPathContext {
    /// Segment 1: the path has not left the form yet.
    Form,
    /// Nothing more resolves here; a further part refuses the path.
    Stop,
    /// A value table or a value tree -- every part is a `<Column id=>`.
    Columns,
    /// A `cfg:DynamicList`.
    DynamicList,
    /// `cfg:ConstantsSet`.
    Constants,
    /// A builtin type and its fixed member table.
    Builtin {
        members: &'static [(&'static str, &'static str)],
        /// Whether it is the settings composer, whose members open the DCS
        /// sub-tree rather than ending the walk.
        composer: bool,
    },
    /// Inside a settings composer, under one parent collection.
    Dcs(&'static [(&'static str, &'static str)]),
    /// A metadata object: the first of `keys` the configuration has, and the
    /// section of it the walk stands in -- a tabular section's name, or
    /// `@std.<Name>` for a standard section such as `ExtDimensionTypes`.
    Meta {
        keys: Vec<String>,
        section: Option<String>,
    },
    /// `<Object>.RegisterRecords`: the next part names a register.
    Registers,
}

/// One part of a `<DataPath>`: its name and the `[<i>]` subscript it carried.
type DataPathToken = (String, Option<String>);

/// Split a `<DataPath>` into parts.
///
/// A leading `~` is dropped -- `~Список` is the attribute `Список` -- and a
/// trailing `[<i>]` is peeled off the part it follows; the walk then emits an
/// extra index segment for it. 2 089 records of the corpus store a different
/// number of segments than the path has parts, and these two spellings plus
/// `Items.…` account for exactly those.
fn parse_data_path_tokens(data_path: &str) -> Vec<DataPathToken> {
    data_path
        .split('.')
        .map(|raw| {
            let raw = raw.trim_start_matches('~');
            match raw.strip_suffix(']').and_then(|head| head.rsplit_once('[')) {
                Some((name, index))
                    if !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit()) =>
                {
                    (name.to_string(), Some(index.to_string()))
                }
                _ => (raw.to_string(), None),
            }
        })
        .collect()
}

/// The members of a builtin attribute type, and whether it is the composer.
fn data_path_builtin_members(
    declared_type: &str,
) -> Option<(&'static [(&'static str, &'static str)], bool)> {
    Some(match declared_type {
        "v8:ValueListType" => (DATA_PATH_VALUE_LIST_MEMBERS, false),
        "v8:StandardPeriod" => (DATA_PATH_STANDARD_PERIOD_MEMBERS, false),
        "v8:StandardBeginningDate" => (DATA_PATH_STANDARD_BEGINNING_DATE_MEMBERS, false),
        "dcsset:SettingsComposer" => (DATA_PATH_COMPOSER_MEMBERS, true),
        "cfg:ReportObject" => (DATA_PATH_REPORT_OBJECT_MEMBERS, true),
        "d4p1:GanttChart" | "d5p1:GanttChart" => (DATA_PATH_GANTT_CHART_MEMBERS, false),
        _ => return None,
    })
}

/// The member table of one settings-composer parent collection.
fn data_path_dcs_members(parent: &str) -> &'static [(&'static str, &'static str)] {
    match parent {
        // A report object's composer opens the composer's own table.
        "SettingsComposer" => DATA_PATH_COMPOSER_MEMBERS,
        "Settings" | "FixedSettings" => DATA_PATH_DCS_SETTINGS,
        "Selection" | "ItemSelection" => DATA_PATH_DCS_SELECTION,
        "GroupFields" | "ItemGroupFields" => DATA_PATH_DCS_GROUP_FIELDS,
        "Filter" | "ItemFilter" => DATA_PATH_DCS_FILTER,
        "ConditionalAppearance" | "ItemConditionalAppearance" => {
            DATA_PATH_DCS_CONDITIONAL_APPEARANCE
        }
        "Order" | "ItemOrder" => DATA_PATH_DCS_ORDER,
        "UserSettings" => DATA_PATH_DCS_USER_SETTINGS,
        "Appearance" => DATA_PATH_DCS_APPEARANCE,
        "FilterAvailableFields" => DATA_PATH_DCS_FILTER_AVAILABLE_FIELDS,
        "Fields" => DATA_PATH_DCS_FIELDS,
        _ => &[],
    }
}

/// A name in a member table.
fn data_path_member(members: &[(&'static str, &'static str)], name: &str) -> Option<&'static str> {
    members
        .iter()
        .find_map(|(member, value)| (*member == name).then_some(*value))
}

/// The standard attributes of an object and of its tabular sections are two
/// tables; a standard section such as `ExtDimensionTypes` is a third.
fn data_path_standard_scope(class: &str, section: Option<&str>) -> String {
    match section {
        None => class.to_string(),
        Some(section) => match section.strip_prefix("@std.") {
            Some(name) => format!("{class}/{name}"),
            None => "<TabularSection>".to_string(),
        },
    }
}

/// The number a standard attribute stores, or `None` when the measured table
/// does not know the name -- which refuses the path rather than defaulting.
fn data_path_standard_attribute(scope: &str, name: &str) -> Option<&'static str> {
    DATA_PATH_STANDARD_ATTRIBUTES
        .iter()
        .find_map(|(entry, member, value)| (*entry == scope && *member == name).then_some(*value))
}

/// The metadata object a declared type names, as `"<Class>.<Name>"`.
fn data_path_object_key(declared_type: &str) -> Option<String> {
    let (head, name) = declared_type.strip_prefix("cfg:")?.split_once('.')?;
    if name.is_empty() {
        return None;
    }
    let class = match head {
        "CatalogObject" | "CatalogRef" | "CatalogList" => "Catalog",
        "DocumentObject" | "DocumentRef" | "DocumentList" => "Document",
        "DataProcessorObject" => "DataProcessor",
        "ReportObject" => "Report",
        "TaskObject" | "TaskRef" => "Task",
        "BusinessProcessObject" | "BusinessProcessRef" => "BusinessProcess",
        "ChartOfCharacteristicTypesObject" | "ChartOfCharacteristicTypesRef" => {
            "ChartOfCharacteristicTypes"
        }
        "ChartOfCalculationTypesObject" | "ChartOfCalculationTypesRef" => "ChartOfCalculationTypes",
        "ChartOfAccountsObject" | "ChartOfAccountsRef" => "ChartOfAccounts",
        "ExchangePlanObject" | "ExchangePlanRef" => "ExchangePlan",
        "InformationRegisterRecordManager"
        | "InformationRegisterRecordSet"
        | "InformationRegisterRecordKey" => "InformationRegister",
        "AccumulationRegisterRecordSet" => "AccumulationRegister",
        "AccountingRegisterRecordSet" => "AccountingRegister",
        "CalculationRegisterRecordSet" => "CalculationRegister",
        "EnumRef" => "Enum",
        "ConstantValueManager" => "Constant",
        _ => return None,
    };
    Some(format!("{class}.{name}"))
}

/// The context a declared type opens.
fn data_path_context_for_types(types: &[String]) -> DataPathContext {
    let Some(first) = types.first().map(String::as_str) else {
        return DataPathContext::Stop;
    };
    match first {
        "v8:ValueTable" | "v8:ValueTree" => return DataPathContext::Columns,
        "cfg:DynamicList" => return DataPathContext::DynamicList,
        "cfg:ConstantsSet" => return DataPathContext::Constants,
        _ => {}
    }
    if let Some((members, composer)) = data_path_builtin_members(first) {
        return DataPathContext::Builtin { members, composer };
    }
    let keys = types
        .iter()
        .filter_map(|declared| data_path_object_key(declared))
        .collect::<Vec<_>>();
    if keys.is_empty() {
        DataPathContext::Stop
    } else {
        DataPathContext::Meta { keys, section: None }
    }
}

/// `data_path_context_for_types`, with a `cfg:DefinedType.<X>` expanded to
/// the type it declares first (rt-paths.md §4.7, 1 of 1).
fn data_path_context_expanding(
    configuration: Option<&dyn ConfigurationObjects>,
    types: &[String],
) -> DataPathContext {
    if let [only] = types
        && let Some(name) = only.trim().strip_prefix("cfg:DefinedType.")
        && let Some(object) = configuration.and_then(|source| source.object(&format!("DefinedType.{name}")))
        && !object.types.is_empty()
    {
        return data_path_context_for_types(&object.types);
    }
    data_path_context_for_types(types)
}

/// What an `Items.<item>` head learns from the item's own `<DataPath>`.
struct DataPathHead<'a> {
    context: DataPathContext,
    attribute: Option<&'a DataPathAttribute>,
}

/// Walk a data path only to learn the context it ends in.
///
/// Used for the head of an `Items.<item>[.CurrentData]` path: the walk carries
/// on in the context of the *item's own* binding.
fn walk_data_path_context<'a>(
    form: &'a DataPathForm,
    configuration: Option<&dyn ConfigurationObjects>,
    tokens: &[DataPathToken],
) -> Option<DataPathHead<'a>> {
    let mut context = DataPathContext::Form;
    let mut attribute: Option<&DataPathAttribute> = None;
    let mut depth = 0usize;
    for (name, _subscript) in tokens {
        if context == DataPathContext::Form {
            let entry = form.attributes.get(name)?;
            attribute = Some(entry);
            context = data_path_context_for_types(&entry.types);
            depth = 1;
            continue;
        }
        let column = if depth == 1 {
            attribute.and_then(|entry| entry.columns.get(name))
        } else {
            None
        };
        if let Some(column) = column {
            context = data_path_context_for_types(&column.types);
            depth += 1;
            continue;
        }
        let next = match &context {
            DataPathContext::Meta { keys, section } => {
                let source = configuration?;
                let object = keys.iter().find_map(|key| source.object(key))?;
                let fields = section
                    .as_deref()
                    .and_then(|section| object.sections.get(section))
                    .unwrap_or(&object.fields);
                let found = fields.get(name)?.first()?;
                Some(if found.tag == "TabularSection" {
                    DataPathContext::Meta {
                        keys: keys.clone(),
                        section: Some(name.clone()),
                    }
                } else {
                    data_path_context_for_types(&found.types)
                })
            }
            _ => None,
        };
        let Some(next) = next else { break };
        context = next;
        depth += 1;
    }
    Some(DataPathHead { context, attribute })
}

/// Member 11 of a field record: what a `<DataPath>` resolves to.
///
/// Returns the whole `{<count>,<segment>,…}` member, or `None` when a part
/// names something the rule cannot place -- an `ExtDimension<N>` of an
/// accounting register, whose uuid appears nowhere in the configuration, or a
/// dynamic list's query field, whose number is the form's own
/// `FieldsMapItemId` and is in no source file.
pub(crate) fn resolve_form_data_path(
    form: &DataPathForm,
    configuration: Option<&dyn ConfigurationObjects>,
    data_path: &str,
) -> Option<String> {
    // `~N.X` -- and `~N.X~N.Y` with a twin -- is the export's spelling of a
    // dynamic-list field it cannot resolve; the terminal part then stores the
    // marked entry of the list's field map.
    let (data_path, marked) = match data_path.trim().strip_prefix('~') {
        Some(body) => match body.split_once('~') {
            Some((body, twin)) => (body, Some(Some(twin.to_string()))),
            None => (body, Some(None)),
        },
        None => (data_path.trim(), None),
    };
    let tokens = parse_data_path_tokens(data_path);
    if tokens.is_empty() || tokens[0].0.is_empty() {
        return None;
    }
    let resolved = resolve_data_path_tokens(form, configuration, &tokens, &marked, 0)?;
    Some(format!(
        "{{{},{}}}",
        resolved.emitted.len(),
        resolved.emitted.join(",")
    ))
}

/// What a walk leaves behind: the segments, the context it ended in, the
/// attribute it stood in and the path expanded to that attribute's root.
struct ResolvedDataPath<'a> {
    emitted: Vec<String>,
    context: DataPathContext,
    attribute: Option<&'a DataPathAttribute>,
    prefix: Vec<String>,
}

fn resolve_data_path_tokens<'a>(
    form: &'a DataPathForm,
    configuration: Option<&dyn ConfigurationObjects>,
    tokens: &[DataPathToken],
    marked: &Option<Option<String>>,
    depth: usize,
) -> Option<ResolvedDataPath<'a>> {
    let mut emitted = Vec::<String>::new();
    let mut context = DataPathContext::Form;
    let mut attribute: Option<&DataPathAttribute> = None;
    let mut prefix = Vec::<String>::new();
    let mut index = 0usize;
    let mut dynamic_list_start = 1usize;

    while index < tokens.len() {
        let (name, subscript) = (tokens[index].0.as_str(), tokens[index].1.as_deref());
        // The context is taken by value so an arm can install the next one;
        // `Stop` is only a placeholder until it does.
        let current = std::mem::replace(&mut context, DataPathContext::Stop);
        let was_dynamic_list = current == DataPathContext::DynamicList;

        if current == DataPathContext::Form {
            if name == "Items" {
                let next = tokens.get(index + 1)?.0.as_str();
                let item = form.items.get(next)?;
                emitted.push(format!("{{{},{FORM_ITEM_NAMESPACE_UUID}}}", item.id));
                index += 2;
                if tokens.get(index).is_some_and(|token| token.0 == "CurrentData") {
                    index += 1;
                }
                // The context after the head is the one the item's own path
                // resolves to, walked in full -- composer levels, builtins and
                // a nested `Items.` head included -- and the additional-column
                // prefix is that path expanded to its attribute (111 holders).
                let inner = parse_data_path_tokens(item.data_path.as_deref()?);
                match (depth < 8)
                    .then(|| resolve_data_path_tokens(form, configuration, &inner, &None, depth + 1))
                    .flatten()
                {
                    Some(head) => {
                        context = head.context;
                        attribute = head.attribute;
                        prefix = head.prefix;
                    }
                    None => {
                        let head = walk_data_path_context(form, configuration, &inner)?;
                        context = head.context;
                        attribute = head.attribute;
                        prefix = inner.into_iter().map(|token| token.0).collect();
                    }
                }
                dynamic_list_start = index;
                continue;
            }
            match form.attributes.get(name) {
                Some(entry) => {
                    attribute = Some(entry);
                    emitted.push(format!("{{{}}}", entry.id));
                    context = data_path_context_expanding(configuration, &entry.types);
                }
                // A handful of items carry the attribute's id where the name
                // should be; the body stores that number unchanged.
                None if !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit()) => {
                    emitted.push(format!("{{{name}}}"));
                    context = DataPathContext::Stop;
                    prefix.push(name.to_string());
                    index += 1;
                    continue;
                }
                None => return None,
            }
        } else {
            let column = if prefix.len() == 1 {
                attribute.and_then(|entry| entry.columns.get(name))
            } else {
                None
            };
            let extra = attribute
                .and_then(|entry| entry.additional_columns.get(&prefix.join(".")))
                .and_then(|columns| columns.get(name));

            if let Some(column) = column {
                emitted.push(format!("{{{}}}", column.id));
                context = data_path_context_for_types(&column.types);
            } else if let Some(extra) = extra.filter(|_| {
                !matches!(
                    current,
                    DataPathContext::Meta { .. } | DataPathContext::DynamicList
                )
            }) {
                emitted.push(format!(
                    "{{{},{DATA_PATH_ADDITIONAL_COLUMN_UUID}}}",
                    extra.id
                ));
                context = data_path_context_for_types(&extra.types);
            } else {
                match current {
                    DataPathContext::DynamicList => {
                        // A dynamic list's own three members are platform
                        // constants; its query fields are not. The number
                        // stored for one is the same form's own
                        // `FieldsMapItemId`, which no `Form.xml` of the corpus
                        // carries -- 0 of 12 507 contain the string at all --
                        // so the path and the FieldsMap are one unknown, not
                        // two, and a writer that cannot produce the one cannot
                        // place the other.
                        let own = (index == dynamic_list_start)
                            .then(|| data_path_member(DATA_PATH_DYNAMIC_LIST_MEMBERS, name))
                            .flatten();
                        if let Some(own) = own {
                            emitted.push(format!("{{{own}}}"));
                            context = if name == "SettingsComposer" {
                                DataPathContext::Builtin {
                                    members: DATA_PATH_COMPOSER_MEMBERS,
                                    composer: true,
                                }
                            } else {
                                DataPathContext::Dcs(data_path_dcs_members(name))
                            };
                            prefix.push(name.to_string());
                            if let Some(subscript) = subscript {
                                emitted.push(format!("{{{subscript},{DATA_PATH_INDEX_UUID}}}"));
                            }
                            index += 1;
                            continue;
                        }
                        if name != "DefaultPicture" || index != dynamic_list_start {
                            // A query field: the id the list's own field map
                            // gives the dotted prefix walked so far.
                            let entry = attribute?;
                            let dotted = tokens[dynamic_list_start..=index]
                                .iter()
                                .map(|token| token.0.as_str())
                                .collect::<Vec<_>>()
                                .join(".");
                            let last = index + 1 == tokens.len();
                            let id = match (marked, last) {
                                (Some(twin), true) => {
                                    let twin = twin.as_ref().map(|twin| {
                                        entry
                                            .list_name
                                            .as_deref()
                                            .and_then(|list| twin.strip_prefix(&format!("{list}.")))
                                            .unwrap_or(twin)
                                            .to_string()
                                    });
                                    entry.dynamic_marked.get(&(dotted, twin))?
                                }
                                _ => entry.dynamic_fields.get(&dotted)?,
                            };
                            emitted.push(format!("{{{id}}}"));
                            context = DataPathContext::DynamicList;
                            if let Some(subscript) = subscript {
                                emitted.push(format!("{{{subscript},{DATA_PATH_INDEX_UUID}}}"));
                            }
                            prefix.push(name.to_string());
                            index += 1;
                            continue;
                        }
                        emitted.push(format!("{{{DATA_PATH_DEFAULT_PICTURE_MARKER}}}"));
                    }
                    DataPathContext::Builtin { members, composer } => {
                        let member = data_path_member(members, name)?;
                        emitted.push(format!("{{{member}}}"));
                        context = if composer {
                            DataPathContext::Dcs(data_path_dcs_members(name))
                        } else {
                            DataPathContext::Stop
                        };
                    }
                    DataPathContext::Dcs(members) => {
                        let member = data_path_member(members, name)?;
                        emitted.push(format!("{{{member}}}"));
                        context = DataPathContext::Dcs(data_path_dcs_members(name));
                    }
                    DataPathContext::Constants => {
                        let object = configuration?.object(&format!("Constant.{name}"))?;
                        emitted.push(format!("{{0,{}}}", object.uuid));
                        context = DataPathContext::Stop;
                    }
                    DataPathContext::Registers => {
                        let source = configuration?;
                        let (key, object) = DATA_PATH_REGISTER_FAMILIES
                            .iter()
                            .map(|family| format!("{family}.{name}"))
                            .find_map(|key| source.object(&key).map(|object| (key, object)))?;
                        emitted.push(format!("{{0,{}}}", object.uuid));
                        context = DataPathContext::Meta {
                            keys: vec![key],
                            section: None,
                        };
                    }
                    DataPathContext::Meta { keys, section } => {
                        let source = configuration?;
                        let object = keys.iter().find_map(|key| source.object(key))?;
                        let (segment, next) = resolve_metadata_data_path_part(
                            source, &object, &keys, &section, name, extra,
                        )?;
                        emitted.push(segment);
                        context = next;
                    }
                    DataPathContext::Form | DataPathContext::Stop | DataPathContext::Columns => {
                        return None;
                    }
                }
            }
        }

        if context == DataPathContext::DynamicList && !was_dynamic_list {
            dynamic_list_start = index + 1;
        }
        if let Some(subscript) = subscript {
            emitted.push(format!("{{{subscript},{DATA_PATH_INDEX_UUID}}}"));
        }
        prefix.push(name.to_string());
        index += 1;
    }

    Some(ResolvedDataPath {
        emitted,
        context,
        attribute,
        prefix,
    })
}

/// One segment resolved against a metadata object, and the context it opens.
///
/// `{0,<uuid>}` is emitted exactly when the segment names something the
/// configuration declares with a `uuid=` attribute, and the uuid is that
/// attribute, verbatim.
fn resolve_metadata_data_path_part(
    configuration: &dyn ConfigurationObjects,
    object: &ConfigurationObject,
    keys: &[String],
    section: &Option<String>,
    name: &str,
    extra: Option<&DataPathColumn>,
) -> Option<(String, DataPathContext)> {
    let fields = section
        .as_deref()
        .and_then(|section| object.sections.get(section))
        .unwrap_or(&object.fields);
    if let Some(found) = fields.get(name) {
        // A chart of accounts names its `<AccountingFlag>` and its
        // `<ExtDimensionAccountingFlag>` alike; `Объект.<name>` is the
        // account's own flag, and the ext-dimension one is reached only
        // through `Объект.ExtDimensionTypes.<name>`.
        let wanted = if section.as_deref() == Some("@std.ExtDimensionTypes") {
            "ExtDimensionAccountingFlag"
        } else {
            "AccountingFlag"
        };
        let pick = if found.len() > 1 {
            found
                .iter()
                .find(|field| field.tag == wanted)
                .or_else(|| found.first())?
        } else {
            found.first()?
        };
        let context = if pick.tag == "TabularSection" {
            DataPathContext::Meta {
                keys: keys.to_vec(),
                section: Some(name.to_string()),
            }
        } else {
            data_path_context_for_types(&pick.types)
        };
        return Some((format!("{{0,{}}}", pick.uuid), context));
    }
    if let Some(total) = name
        .strip_prefix("Total")
        .and_then(|field| fields.get(field))
        .and_then(|found| found.first())
    {
        return Some((
            format!("{{{DATA_PATH_TOTAL_MARKER},{}}}", total.uuid),
            DataPathContext::Stop,
        ));
    }
    // `ExtDimension<N>`, `ExtDimensionDr<N>` and `ExtDimensionCr<N>` of an
    // accounting register are platform constants, one uuid per side over
    // every register, with N-1 in front (81 of 81).
    if object.class == "AccountingRegister" {
        for (prefix, uuid) in [
            ("ExtDimensionDr", "1ab44b24-3315-40a9-b495-f1f1227ac205"),
            ("ExtDimensionCr", "f77758c9-9fcd-490f-9bbd-1e446541f536"),
            ("ExtDimension", "91162600-3161-4326-89a0-4a7cecd5092a"),
        ] {
            if let Some(number) = name
                .strip_prefix(prefix)
                .and_then(|rest| rest.parse::<u32>().ok())
                .filter(|number| *number >= 1)
            {
                return Some((format!("{{{},{uuid}}}", number - 1), DataPathContext::Stop));
            }
        }
    }
    if name == "RowsCount"
        && (section.is_some()
            || matches!(object.class.as_str(), "InformationRegister" | "AccountingRegister"))
    {
        return Some((
            format!("{{{DATA_PATH_ROWS_COUNT_MARKER}}}"),
            DataPathContext::Stop,
        ));
    }
    // `<Dimension>Dr` / `<Dimension>Cr` on an accounting register: the same
    // dimension's uuid, with the side in the first member.
    if object.class == "AccountingRegister"
        && let Some(side) = name
            .strip_suffix("Dr")
            .map(|base| ("2", base))
            .or_else(|| name.strip_suffix("Cr").map(|base| ("3", base)))
        && let Some(base) = fields.get(side.1).and_then(|found| found.first())
    {
        return Some((
            format!("{{{},{}}}", side.0, base.uuid),
            DataPathContext::Stop,
        ));
    }
    if let Some(extra) = extra {
        return Some((
            format!("{{{},{DATA_PATH_ADDITIONAL_COLUMN_UUID}}}", extra.id),
            data_path_context_for_types(&extra.types),
        ));
    }
    if name == "SettingsComposer" {
        return Some((
            format!("{{0,{DATA_PATH_SETTINGS_COMPOSER_UUID}}}"),
            DataPathContext::Builtin {
                members: DATA_PATH_COMPOSER_MEMBERS,
                composer: true,
            },
        ));
    }
    if let Some(common) = configuration.object(&format!("CommonAttribute.{name}")) {
        return Some((format!("{{0,{}}}", common.uuid), DataPathContext::Stop));
    }
    // A standard attribute is a platform constant, and normally ends the walk.
    // Three do not: `Parent` and `Ref` stay on the same object, `Owner` moves
    // to the object's `<Owners>`, and `RegisterRecords` hands the next part to
    // the register it names.
    let scope = data_path_standard_scope(&object.class, section.as_deref());
    let number = data_path_standard_attribute(&scope, name)?;
    let context = match name {
        "RegisterRecords" => DataPathContext::Registers,
        "Parent" | "Ref" => DataPathContext::Meta {
            keys: keys.to_vec(),
            section: None,
        },
        "Owner" if !object.owners.is_empty() => DataPathContext::Meta {
            keys: object.owners.clone(),
            section: None,
        },
        _ if DATA_PATH_STANDARD_SECTIONS.contains(&name) => DataPathContext::Meta {
            keys: keys.to_vec(),
            section: Some(format!("@std.{name}")),
        },
        _ => DataPathContext::Stop,
    };
    Some((format!("{{{number}}}"), context))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three label fields and the one input field of
    /// `Documents/Лот/Forms/ВыигранныеЛоты`, read out of the stored body.
    #[test]
    fn writes_the_field_records_the_platform_stores() {
        let period = format_field_item(&NativeFieldItem {
            id: "20",
            kind: 1,
            name: "ПериодЗакупок",
            functional_options: Some("{0,{0,{\"B\",1},0}}"),
            data_path: "{2,{1},{3}}",
            payload: &format_plain_field_payload(false),
            context_menu: &format_field_context_menu("21", "ПериодЗакупокКонтекстноеМеню", None, &[]),
            extended_tooltip: &format_extended_tooltip("22", "ПериодЗакупокРасширеннаяПодсказка"),
            ..NativeFieldItem::default()
        })
        .expect("a field record");
        assert_eq!(
            period,
            "{37,{20,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},1,\"ПериодЗакупок\",1,0,{1,0},{1,0},{2,{1},{3}},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},\"\",-1,-1,1,0,\"\"},{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{7,3,0,1,100},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{11,0,0,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0},{0,1,0},1,{22,{21,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ПериодЗакупокКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,{\"Pattern\"},{\"Pattern\"},\"\",\"\",{0},0,0,1,{12,{22,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ПериодЗакупокРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},3,3,0,0,0,0}"
        );

        let supplier = format_field_item(&NativeFieldItem {
            id: "46",
            kind: 1,
            name: "АнкетаПоставщика",
            functional_options: Some("{0,{0,{\"B\",1},0}}"),
            data_path: "{1,{2}}",
            payload: &format_plain_field_payload(true),
            context_menu: &format_field_context_menu("47", "АнкетаПоставщикаКонтекстноеМеню", None, &[]),
            extended_tooltip: &format_extended_tooltip("48", "АнкетаПоставщикаРасширеннаяПодсказка"),
            ..NativeFieldItem::default()
        })
        .expect("a field record");
        assert!(
            supplier.contains("{11,0,0,2,2,2,{1,0},1,"),
            "an editable field carries 1 in the slot a label carries 0"
        );
        assert!(supplier.starts_with(
            "{37,{46,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},1,\"АнкетаПоставщика\",1,0,{1,0},{1,0},{1,{2}},"
        ));
        assert!(supplier.ends_with(",3,3,0,0,0,0}"));

        // A field of another kind is the same frame with another kind slot and
        // another payload: the shortest spreadsheet document field of the
        // corpus, without the visibility tuple.
        let result = format_field_item(&NativeFieldItem {
            id: "9",
            kind: 6,
            name: "Результат",
            title_location: Some("None"),
            data_path: "{1,{3}}",
            payload: "{13,100,10,1,1,0,0,1,1,0,0,1,0,0,1,{3,4,{0}},1,1,{0,1,0},0,1,0,0,1,0,0,0,0,1,1,1,2}",
            context_menu: &format_field_context_menu("10", "РезультатКонтекстноеМеню", None, &[]),
            extended_tooltip: &format_extended_tooltip("12", "РезультатРасширеннаяПодсказка"),
            ..NativeFieldItem::default()
        })
        .expect("a field record");
        assert!(result.starts_with(
            "{37,{9,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,6,\"Результат\",0,0,{1,0},{1,0},{1,{3}},{0},1,0,2,"
        ));
        assert!(result.contains(
            ",{13,100,10,1,1,0,0,1,1,0,0,1,0,0,1,{3,4,{0}},1,1,{0,1,0},0,1,0,0,1,0,0,0,0,1,1,1,2},{0,1,0},1,"
        ));
    }

    #[test]
    fn writes_the_default_children_the_platform_stores() {
        assert_eq!(
            format_extended_tooltip("19", "ЛотРасширеннаяПодсказка"),
            "{12,{19,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ЛотРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0}"
        );
        assert_eq!(
            format_field_context_menu("18", "ЛотКонтекстноеМеню", None, &[]),
            "{22,{18,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ЛотКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0}"
        );
    }

    /// The two search buttons of
    /// `Documents/Лот/Forms/ВыигранныеЛоты`'s list command bar, and the bar
    /// itself, exactly as that body stores them.
    #[test]
    fn writes_the_command_bar_records_the_platform_stores() {
        assert_eq!(
            format_standard_command_button(
                "26",
                "ФормаНайти",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "27",
                "ФормаНайтиРасширеннаяПодсказка",
            ),
            "{31,{26,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,{0,{0,{\"B\",1},0}},0,\"ФормаНайти\",{1,0},1,{1,c0519548-2a9a-44de-a25e-faf01e089d4d},{0},3,0,0,0,2,2,0,0,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},0,{4,0,{0},\"\",-1,-1,1,0,\"\"},1,{\"Pattern\"},\"\",2,0,1,{12,{27,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ФормаНайтиРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},{\"U\"},1,0,0,1,0,0,0,3,3,3,0,0,0,0,0,0,1,0}"
        );
        assert_eq!(
            format_standard_command_button(
                "28",
                "ФормаОтменитьПоиск",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "29",
                "ФормаОтменитьПоискРасширеннаяПодсказка",
            ),
            "{31,{28,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,{0,{0,{\"B\",1},0}},0,\"ФормаОтменитьПоиск\",{1,0},1,{1,44ad3ec9-f3c2-4913-9224-5f9fb6418743},{0},3,0,0,0,2,2,0,0,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},0,{4,0,{0},\"\",-1,-1,1,0,\"\"},1,{\"Pattern\"},\"\",2,0,1,{12,{29,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ФормаОтменитьПоискРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},{\"U\"},1,0,0,1,0,0,0,3,3,3,0,0,0,0,0,0,1,0}"
        );
        assert_eq!(
            format_empty_auto_command_bar("3", "СписокКоманднаяПанель"),
            "{22,{3,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"СписокКоманднаяПанель\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0}"
        );
    }

    /// Two decorations of
    /// `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов`, and the
    /// tooltip shape an item with a visibility tuple carries, exactly as those
    /// bodies store them.
    #[test]
    fn writes_the_decoration_records_the_platform_stores() {
        assert_eq!(
            format_label_decoration(&NativeLabelDecoration {
                id: "1195",
                name: "ДекорацияВниманиеТолькоПросмотр",
                title: &format_russian_title("Внимание"),
                picture_uuid: Some("188d8f0e-94da-44bb-8ed3-21aa01e973b9"),
                context_menu_id: "1196",
                context_menu_name: "ДекорацияВниманиеТолькоПросмотрКонтекстноеМеню",
                extended_tooltip_id: "1197",
                extended_tooltip_name: "ДекорацияВниманиеТолькоПросмотрРасширеннаяПодсказка",
            }),
            "{12,{1195,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,1,\"ДекорацияВниманиеТолькоПросмотр\",{1,1,{\"ru\",\"Внимание\"}},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{4,{4,1,{0,188d8f0e-94da-44bb-8ed3-21aa01e973b9},\"\",-1,-1,0,0,\"\"},0,0,0,{1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},0,0,{0,1,0},0,100},1,{22,{1196,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ДекорацияВниманиеТолькоПросмотрКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,2,{1,{1,1,{\"ru\",\"Внимание\"}},0},0,1,{12,{1197,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ДекорацияВниманиеТолькоПросмотрРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},1,0,0,1,0,3,3,0,0}"
        );

        assert_eq!(
            format_visible_extended_tooltip("6", "СценарийРасширеннаяПодсказка"),
            "{12,{6,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},0,\"СценарийРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0}"
        );

        assert_eq!(format_russian_title(""), "{1,0}");
        assert_eq!(format_russian_title("Внимание"), "{1,1,{\"ru\",\"Внимание\"}}");
    }

    /// One childless record of each group kind, exactly as an ERP УХ form
    /// body stores it. The six together prove the frame: everything outside
    /// the payload is the same in all of them.
    #[test]
    fn writes_one_record_of_every_group_kind() {
        let cases: [(u8, &str, &str, &str, &str, &str, &str, &str); 6] = [
            (
                0,
                "76",
                "ГруппаКоманднаяПанель",
                "Командная панель",
                "{1,0,{0,02023637-7868-4a5f-8576-835a76e0c9ba}}",
                "77",
                "ГруппаКоманднаяПанельРасширеннаяПодсказка",
                "{22,{76,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ГруппаКоманднаяПанель\",{1,1,{\"ru\",\"Командная панель\"}},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,0,{0,02023637-7868-4a5f-8576-835a76e0c9ba}},0,1,0,1,",
            ),
            (
                2,
                "461",
                "ГруппаВидКвоты",
                "",
                "{2,2,1,0,3,{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{0},{\"Pattern\"},\"\",{1,0},0}",
                "462",
                "ГруппаВидКвотыРасширеннаяПодсказка",
                "{22,{461,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,2,\"ГруппаВидКвоты\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{2,2,1,0,3,{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{0},{\"Pattern\"},\"\",{1,0},0},0,1,0,1,",
            ),
            (
                5,
                "11",
                "ГруппаШапка",
                "",
                "{29,1,0,2,1,{0},{1,0},{\"Pattern\"},\"\",{3,4,{0}},0,0,0,1,{1,0},0,0,3,3,2,0,1,2,{3,4,{0}},0,2,0,2,0}",
                "50",
                "ГруппаШапкаРасширеннаяПодсказка",
                "{22,{11,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,\"ГруппаШапка\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{29,1,0,2,1,{0},{1,0},{\"Pattern\"},\"\",{3,4,{0}},0,0,0,1,{1,0},0,0,3,3,2,0,1,2,{3,4,{0}},0,2,0,2,0},0,1,0,1,",
            ),
            (
                6,
                "193",
                "ГруппаКнопокДляПрисоединенныхФайлов",
                "",
                "{2,{0},2,0}",
                "194",
                "ГруппаКнопокДляПрисоединенныхФайловРасширеннаяПодсказка",
                "{22,{193,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,6,\"ГруппаКнопокДляПрисоединенныхФайлов\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{2,{0},2,0},0,1,0,1,",
            ),
            (
                1,
                "482",
                "ПодменюПечать",
                "",
                "{7,{4,1,{-13},\"\",-1,-1,1,0,\"\"},{0},2,3,0,0,{3,4,{0}},{3,4,{0}}}",
                "483",
                "ПодменюПечатьРасширеннаяПодсказка",
                "",
            ),
            (
                4,
                "375",
                "ШаблоныСтраница",
                "",
                "{18,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,{0},{1,0},1,{\"Pattern\"},\"\",{3,4,{0}},0,0,3,3,0,0,0,0,{3,4,{0}},{7,3,0,1,100}}",
                "376",
                "ШаблоныСтраницаРасширеннаяПодсказка",
                "",
            ),
        ];

        for (kind, id, name, title, payload, tooltip_id, tooltip_name, expected_head) in cases {
            let tooltip = format_extended_tooltip(tooltip_id, tooltip_name);
            let record = format_group_item(&NativeGroupItem {
                id,
                kind,
                name,
                title: &format_russian_title(title),
                tooltip_title: "{1,0}",
                payload,
                children: &[],
                extended_tooltip: Some(&tooltip),
                ..NativeGroupItem::default()
            })
            .expect("a group record");
            if !expected_head.is_empty() {
                assert!(
                    record.starts_with(expected_head),
                    "kind {kind} head differs:\n{record}"
                );
            }
            assert!(record.ends_with(&format!("{tooltip},0,3,3,0}}")));
        }
    }

    /// A table head of ERP УХ, exactly as stored. The form names eight
    /// properties and the writer reads every one of them.
    #[test]
    fn writes_the_table_heads_the_platform_stores() {
        let head = format_table_head(&NativeTableHead {
            id: "29",
            representation: Some("List"),
            name: "Список",
            title: "{1,2,{\"ru\",\"Список\"},{\"en\",\"List\"}}",
            data_path: "{1,{7}}",
            header: false,
            horizontal_lines: false,
            vertical_lines: false,
            auto_insert_new_row: true,
            row_picture_data_path: "{1,{3}}",
            ..NativeTableHead::default()
        })
        .expect("a table head");
        assert_eq!(head, "55,{29,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,\"Список\",0,0,1,{1,2,{\"ru\",\"Список\"},{\"en\",\"List\"}},{1,0},{1,{7}},0,1,0,0,0,1,1,0,0,0,0,0,1,0,0,1,0,1,2,2,0,0,0,0,0,1,2,0,0,1,1,{1,{3}},{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{3,4,{0}},{7,3,0,1,100},{0,0,0},0,0");
        assert_eq!(top_level_members(&format!("{{{head}}}")).len(), 54);

        // The record puts the bag, the events, the two children and the
        // columns between the head and the tail.
        let record = format_table_record(
            &head,
            &[("5", "{\"B\",0}".to_string())],
            "{0,1,0}",
            "{0}",
            "{22,{30,x},0}",
            "{22,{31,x},0}",
            &[("77ffcc29-7f2d-4223-b22f-19666e7250ba", "{37,{32,x},0}".to_string())],
            "2,2,1,0",
        );
        assert!(record.contains(",1,5,{\"B\",0},{0,1,0},{0},1,{22,{30,x},0},1,{22,{31,x},0},1,77ffcc29-"));
        assert!(record.ends_with(",{37,{32,x},0},2,2,1,0}"));

        // A spelling the corpus never showed is refused.
        assert_eq!(
            format_table_head(&NativeTableHead {
                representation: Some("Chart"),
                ..NativeTableHead::default()
            }),
            None
        );
    }

    /// What a table says about itself after its columns.
    #[test]
    fn writes_what_a_table_says_about_itself() {
        let quiet = format_table_tail(&NativeTableTail {
            extended_tooltip: "{12,{6,x},0}",
            additions: ["{5,{7,x},0}", "{5,{10,x},0}", "{5,{13,x},0}"],
            ..NativeTableTail::default()
        })
        .expect("a table tail");
        assert_eq!(top_level_members(&format!("{{{quiet}}}")).len(), 37);
        assert!(quiet.starts_with("2,2,1,0,{\"Pattern\"},\"\",\"\",2,2,0,1,{12,{6,x},0},0,0,0,1,"));
        // A table that names no drag mode writes 1 there, not 0.
        assert!(quiet.ends_with(",3,3,0,1,0,0,0,0,1,0"));

        let spoken = format_table_tail(&NativeTableTail {
            auto_mark_incomplete: Some(true),
            auto_add_incomplete: Some(false),
            visible: false,
            multiple_choice: true,
            skip_on_input: Some(true),
            search_on_input: Some("DontUse"),
            tooltip_representation: Some("ShowRight"),
            extended_tooltip: "{12,{6,x},0}",
            search_string_location: Some("PullFromTop"),
            view_status_location: Some("Top"),
            search_control_location: Some("CommandBar"),
            additions: ["{5,{7,x},0}", "{5,{10,x},0}", "{5,{13,x},0}"],
            refresh_request: Some("PullFromTop"),
            auto_max_width: false,
            max_width: Some("70"),
            auto_max_height: false,
            max_height: Some("12"),
            height_control_variant: Some("UseContentHeight"),
            auto_max_rows_count: false,
            max_rows_count: Some("6"),
            current_row_use: Some("SelectionPresentationAndChoice"),
            behavior_on_horizontal_compression: None,
            file_drag_mode: Some("AsFile"),
            group_horizontal_align: None,
            group_vertical_align: None,
            display_importance: "0",
        })
        .expect("a table tail");
        assert!(spoken.starts_with("1,0,0,1,{\"Pattern\"},\"\",\"\",1,1,8,1,{12,{6,x},0},6,2,2,1,"));
        assert!(spoken.ends_with(",1,0,70,0,0,12,3,3,3,0,6,3,0,0,0,0"));

        // A spelling the corpus never showed is refused.
        assert_eq!(
            format_table_tail(&NativeTableTail {
                search_on_input: Some("Sometimes"),
                ..NativeTableTail::default()
            }),
            None
        );
    }

    /// What a decoration says about itself. The sharpest check: the full
    /// writer with its defaults reproduces, member for member, what the narrow
    /// `format_extended_tooltip` writes -- and that shape was read off the
    /// bodies independently.
    #[test]
    fn writes_what_a_decoration_says_about_itself() {
        assert_eq!(
            format_decoration_item(&NativeDecorationItem {
                id: "22",
                name: "ПериодЗакупокРасширеннаяПодсказка",
                ..NativeDecorationItem::default()
            })
            .as_deref(),
            Some(format_extended_tooltip("22", "ПериодЗакупокРасширеннаяПодсказка").as_str())
        );

        // The functional-options block goes inline after the flag, and the
        // two stretches read 2 when the decoration names neither value.
        let restricted = format_decoration_item(&NativeDecorationItem {
            id: "22",
            functional_options: Some("{0,{0,{\"B\",1},0}}"),
            name: "Подсказка",
            horizontal_stretch: Some(false),
            width: Some("40"),
            auto_max_width: false,
            max_width: Some("60"),
            ..NativeDecorationItem::default()
        })
        .expect("a decoration record");
        assert!(restricted.starts_with(
            "{12,{22,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},0,\"Подсказка\",{1,0},{1,0},1,40,0,0,2,"
        ));
        assert!(restricted.ends_with(",0,0,60,0,1,0,3,3,0,0}"));

        // A label decoration is the same record carrying two children a
        // tooltip never has, each behind its own flag.
        let labelled = format_decoration_item(&NativeDecorationItem {
            id: "8",
            name: "Надпись",
            context_menu: Some("{22,{9,x},0}"),
            visible: false,
            skip_on_input: Some(true),
            tooltip_representation: Some("ShowBottom"),
            extended_tooltip: Some("{12,{10,x},0}"),
            enabled: false,
            ..NativeDecorationItem::default()
        })
        .expect("a decoration record");
        assert!(labelled.contains(",\"Надпись\",{1,0},{1,0},0,0,0,2,2,"));
        assert!(labelled.contains(",1,{22,{9,x},0},0,1,{1,{1,0},0},7,1,{12,{10,x},0},1,0,0,1,0,3,3,0,0}"));
        assert_eq!(top_level_members(&labelled).len(), 36);

        // A picture decoration is the same record with kind 1.
        let picture = format_decoration_item(&NativeDecorationItem {
            kind: 1,
            ..NativeDecorationItem::default()
        })
        .expect("a decoration record");
        assert!(picture.contains(",0,0,1,\"\","));

        // A spelling the corpus never showed is refused.
        assert_eq!(
            format_decoration_item(&NativeDecorationItem {
                group_vertical_align: Some("Stretch"),
                ..NativeDecorationItem::default()
            }),
            None
        );
    }

    /// The `{5,…}` payload of a label decoration, against four payloads read
    /// out of stored ERP УХ bodies.
    #[test]
    fn writes_the_label_decoration_payload_the_corpus_stores() {
        // The default is what a tooltip and 23 380 of the 35 477 ERP УХ label
        // decorations carry -- and what the writer used to give all of them.
        assert_eq!(
            format_label_decoration_payload(&NativeLabelDecorationPayload::default()).as_deref(),
            Some(
                "{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},\
                 {3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}}"
            )
        );

        // `BusinessProcesses/ТиповаяПродажа/Forms/Взаимодействия` item 183:
        // `<Hyperlink>true</>`, `<HorizontalAlign>Right</>` and one `Click`.
        let events = format_native_events(
            "LabelDecoration",
            "",
            &[NativeEvent {
                name: "Click",
                handler: "ДекорацияВзаимодействияНажатие",
            }],
        )
        .expect("the click of a label decoration");
        assert_eq!(
            format_label_decoration_payload(&NativeLabelDecorationPayload {
                hyperlink: true,
                horizontal_align: Some("Right"),
                events: &events,
                ..NativeLabelDecorationPayload::default()
            })
            .as_deref(),
            Some(
                "{5,1,2,3,0,{1,11707a99-4eb9-4373-bc8c-84891483a034,\
                 \"ДекорацияВзаимодействияНажатие\",1,0,11707a99-4eb9-4373-bc8c-84891483a034,0,1},\
                 {3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}}"
            )
        );

        // `Catalogs/ПоказателиМонитораКлючевыхПоказателей/Forms/ФормаЭлемента`
        // item 246: `<BackColor>#C0DCC0</>` at member 6 and a `Single`
        // `<Border>` at member 8.
        let border = format_native_control_border(Some("Single"), "1").expect("a single border");
        assert_eq!(
            format_label_decoration_payload(&NativeLabelDecorationPayload {
                back_color: "{3,0,{12639424}}",
                border: &border,
                ..NativeLabelDecorationPayload::default()
            })
            .as_deref(),
            Some(
                "{5,0,0,3,0,{0,1,0},{3,0,{12639424}},{3,4,{0}},\
                 {3,0,{0},1,1,0,48312c09-257f-4b29-b280-284dd89efc1e}}"
            )
        );

        // `Catalogs/НастройкиОнлайнОплат/Forms/ФормаЭлемента` item 280:
        // `<VerticalAlign>Center</>` at member 3 and `<TitleHeight>1</>` at 4.
        assert_eq!(
            format_label_decoration_payload(&NativeLabelDecorationPayload {
                vertical_align: Some("Center"),
                title_height: Some("1"),
                ..NativeLabelDecorationPayload::default()
            })
            .as_deref(),
            Some(
                "{5,0,0,1,1,{0,1,0},{3,4,{0}},{3,4,{0}},\
                 {3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}}"
            )
        );

        // Neither corpus stores `<HorizontalAlign>Left` or `<VerticalAlign>Auto`
        // anywhere, so nothing says what they would write.
        assert_eq!(
            format_label_decoration_payload(&NativeLabelDecorationPayload {
                horizontal_align: Some("Left"),
                ..NativeLabelDecorationPayload::default()
            }),
            None
        );
        assert_eq!(
            format_label_decoration_payload(&NativeLabelDecorationPayload {
                vertical_align: Some("Auto"),
                ..NativeLabelDecorationPayload::default()
            }),
            None
        );
        assert_eq!(format_native_control_border(Some("Dotted"), "1"), None);
    }

    /// The `{4,…}` payload of a picture decoration, against three payloads read
    /// out of stored ERP УХ bodies. It is a **different shape** from the
    /// label's, which is why the constant the writer used to emit was wrong for
    /// all 7 222 picture decorations of that corpus.
    #[test]
    fn writes_the_picture_decoration_payload_the_corpus_stores() {
        // `Catalogs/БланкиОтчетов/Forms/МастерСозданияНовыхСтрок` item 85:
        // a common picture loaded transparent, `<PictureSize>Stretch</>` and
        // `<FileDragMode>AsFile</>`, which writes **0** where naming nothing
        // writes 1.
        assert_eq!(
            format_picture_decoration_payload(&NativePictureDecorationPayload {
                picture: &format_native_item_picture(
                    Some("408c0202-799d-4e99-9e0f-b81c69f64b5a"),
                    true,
                    None,
                    None,
                ),
                picture_size: Some("Stretch"),
                file_drag_mode: Some("AsFile"),
                ..NativePictureDecorationPayload::default()
            })
            .as_deref(),
            Some(
                "{4,{4,1,{0,408c0202-799d-4e99-9e0f-b81c69f64b5a},\"\",-1,-1,1,0,\"\"},0,1,0,\
                 {1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},0,0,\
                 {0,1,0},0,100}"
            )
        );

        // `Catalogs/БланкиОтчетов/Forms/СопоставлениеАналитике` item 60:
        // `<xr:TransparentPixel x="12" y="3"/>` is members 4 and 5 of the
        // reference, where absent writes -1.
        assert_eq!(
            format_native_item_picture(
                Some("fcb2acfb-de4e-4c67-ac6f-c386ed317b5f"),
                true,
                Some("12"),
                Some("3"),
            ),
            "{4,1,{0,fcb2acfb-de4e-4c67-ac6f-c386ed317b5f},\"\",12,3,1,0,\"\"}"
        );
        // No picture at all still writes 1 at member 6.
        assert_eq!(
            format_native_item_picture(None, false, None, None),
            "{4,0,{0},\"\",-1,-1,1,0,\"\"}"
        );

        // `Catalogs/РесурсныеСпецификации/Forms/ФормаЭлемента` item 1712:
        // `<Hyperlink>`, a `Single` `<Border>` and a `Click`, whose uuid is the
        // picture decoration's own and not the label decoration's.
        let events = format_native_events(
            "PictureDecoration",
            "",
            &[NativeEvent {
                name: "Click",
                handler: "ГиперссылкаНадписьОсновноеИзделиеНажатие",
            }],
        )
        .expect("the click of a picture decoration");
        let border = format_native_control_border(Some("Single"), "1").expect("a single border");
        assert_eq!(
            format_picture_decoration_payload(&NativePictureDecorationPayload {
                picture: &format_native_item_picture(
                    Some("ce463861-92df-47f2-bd0f-4a2dbf72aacc"),
                    false,
                    None,
                    None,
                ),
                hyperlink: true,
                border: &border,
                events: &events,
                file_drag_mode: Some("AsFile"),
                ..NativePictureDecorationPayload::default()
            })
            .as_deref(),
            Some(
                "{4,{4,1,{0,ce463861-92df-47f2-bd0f-4a2dbf72aacc},\"\",-1,-1,0,0,\"\"},1,0,0,\
                 {1,0},{3,4,{0}},{3,0,{0},1,1,0,48312c09-257f-4b29-b280-284dd89efc1e},0,0,\
                 {1,9874537f-454c-40ae-83e9-3b9cefbc6d08,\
                 \"ГиперссылкаНадписьОсновноеИзделиеНажатие\",1,0,\
                 9874537f-454c-40ae-83e9-3b9cefbc6d08,0,1},0,100}"
            )
        );

        // `<PictureSize>RealSize` is never stored, so it is refused rather than
        // taking the 0 an absent `<PictureSize>` writes.
        assert_eq!(
            format_picture_decoration_payload(&NativePictureDecorationPayload {
                picture_size: Some("RealSize"),
                ..NativePictureDecorationPayload::default()
            }),
            None
        );
    }

    /// What a button says about itself, read off the 77 127 `{31,…}` records
    /// of ERP УХ.
    #[test]
    fn writes_what_a_button_says_about_itself() {
        // The shape the narrow writer produces for a standard-command button
        // is what the full one produces from the same facts, member for
        // member -- which is the sharpest check available on the defaults.
        let tooltip = format_extended_tooltip("27", "ФормаНайтиРасширеннаяПодсказка");
        assert_eq!(
            format_button_item(&NativeButtonItem {
                id: "26",
                functional_options: Some("{0,{0,{\"B\",1},0}}"),
                name: "ФормаНайти",
                command: "{1,c0519548-2a9a-44de-a25e-faf01e089d4d}",
                extended_tooltip: &tooltip,
                ..NativeButtonItem::default()
            })
            .as_deref(),
            Some(
                format_standard_command_button(
                    "26",
                    "ФормаНайти",
                    "c0519548-2a9a-44de-a25e-faf01e089d4d",
                    "27",
                    "ФормаНайтиРасширеннаяПодсказка",
                )
                .as_str()
            )
        );

        // <Type> and <LocationInCommandBar> are each written twice, and the
        // second coding is finer than the first: a command-bar hyperlink is a
        // command-bar button at member 4 and itself at member 46.
        let members = |button: &NativeButtonItem<'_>| {
            top_level_members(&format_button_item(button).expect("a button record"))
        };
        let hyperlink = members(&NativeButtonItem {
            button_type: Some("CommandBarHyperlink"),
            location_in_command_bar: Some("InCommandBarAndInAdditionalSubmenu"),
            ..NativeButtonItem::default()
        });
        assert_eq!(hyperlink[4], "0");
        assert_eq!(hyperlink[46], "3");
        assert_eq!(hyperlink[15], "1");
        assert_eq!(hyperlink[49], "3");

        let submenu = members(&NativeButtonItem {
            button_type: Some("UsualButton"),
            location_in_command_bar: Some("InAdditionalSubmenu"),
            ..NativeButtonItem::default()
        });
        assert_eq!(submenu[4], "1");
        assert_eq!(submenu[46], "1");
        assert_eq!(submenu[15], "0");
        assert_eq!(submenu[49], "1");

        // A spelling the corpus never showed is refused rather than defaulted.
        assert_eq!(
            format_button_item(&NativeButtonItem {
                button_type: Some("Toggle"),
                ..NativeButtonItem::default()
            }),
            None
        );
        assert_eq!(
            format_button_item(&NativeButtonItem {
                shape: Some("Round"),
                ..NativeButtonItem::default()
            }),
            None
        );
    }

    /// What a field says about itself, read off the 69 521 `{37,…}` input
    /// field records of ERP УХ.
    #[test]
    fn writes_what_a_field_says_about_itself() {
        // A field that says nothing takes every absent value: the title
        // location is 1, the two tri-states are 2, the aligns are 3 except the
        // header's, which is 0.
        let plain = format_field_item(&NativeFieldItem {
            id: "4",
            name: "Поле",
            data_path: "{1,{2}}",
            payload: "{36,{3,0}}",
            context_menu: "{22,{5,x},0}",
            extended_tooltip: "{12,{6,x},0}",
            ..NativeFieldItem::default()
        })
        .expect("a field record");
        assert!(plain.starts_with(
            "{37,{4,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,2,\"Поле\",1,0,{1,0},{1,0},{1,{2}},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,"
        ));
        assert!(plain.ends_with(",1,{12,{6,x},0},3,3,0,0,0,0}"));

        // Every member the source decides, spoken.
        let spoken = format_field_item(&NativeFieldItem {
            id: "4",
            name: "Поле",
            title_location: Some("Right"),
            title_height: Some("2"),
            data_path: "{1,{2}}",
            enabled: false,
            read_only: true,
            skip_on_input: Some(true),
            default_item: true,
            warning_on_edit: Some("DontShow"),
            show_in_header: false,
            show_in_footer: false,
            cell_hyperlink: true,
            horizontal_align: Some("Right"),
            header_horizontal_align: Some("Center"),
            footer_horizontal_align: Some("Left"),
            vertical_align: Some("Bottom"),
            edit_mode: Some("EnterOnInput"),
            auto_cell_height: true,
            payload: "{36,{3,0}}",
            context_menu: "{22,{5,x},0}",
            visible: false,
            fixing_in_table: Some("Left"),
            tooltip_representation: Some("ShowRight"),
            extended_tooltip: "{12,{6,x},0}",
            group_horizontal_align: Some("Center"),
            group_vertical_align: Some("Top"),
            ..NativeFieldItem::default()
        })
        .expect("a field record");
        assert!(spoken.contains(",\"Поле\",4,2,{1,0},{1,0},{1,{2}},{0},0,1,1,1,1,{1,0},{1,0},0,0,1,2,1,0,2,2,1,"));
        assert!(spoken.contains(",{0},1,8,1,{12,{6,x},0},1,0,0,0,0,0}"));
        assert!(spoken.contains(",{22,{5,x},0},0,"));

        // A spelling the corpus never showed is refused rather than defaulted.
        assert_eq!(
            format_field_item(&NativeFieldItem {
                title_location: Some("Inline"),
                ..NativeFieldItem::default()
            }),
            None
        );
        assert_eq!(
            format_field_item(&NativeFieldItem {
                fixing_in_table: Some("Both"),
                ..NativeFieldItem::default()
            }),
            None
        );

        // Every kind, by the element that names it.
        assert_eq!(native_field_kind("LabelField"), Some(1));
        assert_eq!(native_field_kind("InputField"), Some(2));
        assert_eq!(native_field_kind("CheckBoxField"), Some(3));
        assert_eq!(native_field_kind("PictureField"), Some(4));
        assert_eq!(native_field_kind("RadioButtonField"), Some(5));
        assert_eq!(native_field_kind("SpreadSheetDocumentField"), Some(6));
        assert_eq!(native_field_kind("TextDocumentField"), Some(7));
        assert_eq!(native_field_kind("ProgressBarField"), Some(9));
        assert_eq!(native_field_kind("GanttChartField"), Some(12));
        assert_eq!(native_field_kind("HTMLDocumentField"), Some(15));
        assert_eq!(native_field_kind("FormattedDocumentField"), Some(17));
        assert_eq!(native_field_kind("UsualGroup"), None);
    }

    /// What a container says about itself, read off the 9 357 `{22,…}` records
    /// of the corpus.
    #[test]
    fn writes_what_a_container_says_about_itself() {
        // Every kind, by the element that names it.
        assert_eq!(native_group_kind("CommandBar"), Some(0));
        assert_eq!(native_group_kind("Popup"), Some(1));
        assert_eq!(native_group_kind("ColumnGroup"), Some(2));
        assert_eq!(native_group_kind("Pages"), Some(3));
        assert_eq!(native_group_kind("Page"), Some(4));
        assert_eq!(native_group_kind("UsualGroup"), Some(5));
        assert_eq!(native_group_kind("ButtonGroup"), Some(6));
        assert_eq!(native_group_kind("ContextMenu"), Some(8));
        assert_eq!(native_group_kind("AutoCommandBar"), Some(9));
        assert_eq!(native_group_kind("InputField"), None);

        // A group that says nothing: member 4 is the flag with nothing after
        // it, the stretches are 2, and the tail closes 1,0,0,0,3,3,0.
        let plain = format_group_item(&NativeGroupItem {
            id: "7",
            name: "Группа",
            payload: "{1,0,{0}}",
            ..NativeGroupItem::default()
        })
        .expect("a group record");
        assert!(plain.starts_with("{22,{7,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,\"Группа\",{1,0},{1,0},0,1,0,0,0,2,2,"));
        assert!(plain.ends_with(",1,{1,0,{0}},0,1,0,0,0,3,3,0}"));

        // The functional-options block goes inline after the flag, before the
        // kind -- which is what makes the head variable-length.
        let restricted = format_group_item(&NativeGroupItem {
            id: "7",
            name: "Группа",
            functional_options: Some("{0,{0,{\"B\",1},0}}"),
            payload: "{1,0,{0}}",
            ..NativeGroupItem::default()
        })
        .expect("a group record");
        assert!(restricted.starts_with(
            "{22,{7,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},5,\"Группа\","
        ));

        // What the source decides, one by one.
        let spoken = format_group_item(&NativeGroupItem {
            id: "7",
            name: "Группа",
            enable_content_change: true,
            enabled: false,
            read_only: true,
            width: Some("40"),
            height: Some("12"),
            horizontal_stretch: Some(true),
            vertical_stretch: Some(false),
            payload: "{1,0,{0}}",
            visible: false,
            tooltip_representation: Some("ShowBottom"),
            horizontal_align: Some("Right"),
            vertical_align: Some("Top"),
            ..NativeGroupItem::default()
        })
        .expect("a group record");
        assert!(spoken.contains(",\"Группа\",{1,0},{1,0},1,0,1,40,12,1,0,"));
        assert!(spoken.ends_with(",0,7,0,0,2,0,0}"));

        // A spelling the corpus never showed is refused rather than defaulted.
        assert_eq!(
            format_group_item(&NativeGroupItem {
                tooltip_representation: Some("Flyover"),
                ..NativeGroupItem::default()
            }),
            None
        );
    }

    /// A group writes its children as `(group uuid, record)` pairs after their
    /// count, which is what the list command bar of
    /// `Documents/Лот/Forms/ВыигранныеЛоты` stores.
    #[test]
    fn writes_a_group_with_its_children() {
        let tooltip = format_extended_tooltip("9", "ГруппаРасширеннаяПодсказка");
        let child = format_standard_command_button(
            "26",
            "ФормаНайти",
            "c0519548-2a9a-44de-a25e-faf01e089d4d",
            "27",
            "ФормаНайтиРасширеннаяПодсказка",
        );
        let record = format_group_item(&NativeGroupItem {
            id: "8",
            kind: 0,
            name: "Группа",
            title: "{1,0}",
            tooltip_title: "{1,0}",
            payload: "{1,0,{0}}",
            children: &[("a9f3b1ac-f51b-431e-b102-55a69acdecad", child.clone())],
            extended_tooltip: Some(&tooltip),
            ..NativeGroupItem::default()
        })
        .expect("a group record");

        assert!(record.contains(&format!(
            ",1,a9f3b1ac-f51b-431e-b102-55a69acdecad,{child},1,0,1,"
        )));
    }

    /// An attribute and a command of ERP УХ form bodies, exactly as they are
    /// stored. Both are wrapper 9; only the namespace in the id tuple, and the
    /// member count that follows from it, tell them apart.
    #[test]
    fn writes_the_attribute_and_command_records_the_platform_stores() {
        assert_eq!(
            format_form_attribute(&NativeFormAttribute {
                id: "9",
                name: "ЦветаФона",
                ..NativeFormAttribute::default()
            }),
            "{9,{9},0,\"ЦветаФона\",{1,0},{\"Pattern\"},{0,{0,{\"B\",1},0}},{0,{0,{\"B\",1},0}},{0,0},{0,0},0,0,0,0,{0,0},{0,0}}"
        );

        // The form's main attribute, saved and fill-checked, with two columns:
        // member 13 counts them and the record grows by exactly that many.
        let columns = [
            "{5,1,0,\"Представление\",{1,0},{\"Pattern\",{\"S\",100,1}},{0,{0,{\"B\",1},0}},{0,{0,{\"B\",1},0}},{0,0},0}"
                .to_string(),
            "{5,2,0,\"Графа\",{1,0},{\"Pattern\",{\"S\"}},{0,{0,{\"B\",1},0}},{0,{0,{\"B\",1},0}},{0,0},0}"
                .to_string(),
        ];
        let record = format_form_attribute(&NativeFormAttribute {
            id: "1",
            name: "Объект",
            main_attribute: true,
            saved_data: true,
            fill_check: true,
            columns: &columns,
            ..NativeFormAttribute::default()
        });
        assert!(record.contains(",{0,0},{0,0},1,1,1,2,{5,1,0,"));
        assert_eq!(top_level_members(&record).len(), 16 + columns.len());
        assert!(record.ends_with(",{0,0},{0,0}}"));
        assert_eq!(
            format_form_command(&NativeFormCommand {
                id: "10",
                name: "Команда9",
                picture_index: "{0,57,0}",
                action: "Команда9",
                current_row_use: Some("DontUse"),
                ..NativeFormCommand::default()
            })
            .as_deref(),
            Some("{9,{10,409b9a53-7f7e-4178-86c1-33176c7c7a7a},\"Команда9\",{1,0},{1,0},{0,{0,{\"B\",1},0}},{0,57,0},{4,0,{0},\"\",-1,-1,1,0,\"\"},\"Команда9\",3,0,0,{0,0},1,0,1,0,0,1}")
        );

        // A value table's own columns, verbatim out of
        // `AccumulationRegisters\ОстаткиПартийЗЕРНО\Forms\ФормаВыбораЗначений`,
        // and one that names a functional option out of
        // `…\РНПТМатериаловВПроизводстве\Forms\ПодборПоСпецификации`.
        assert_eq!(
            format_form_attribute_column(&NativeFormAttributeColumn {
                id: "4",
                name: "ОрганизацияКонтрагентСтрокой",
                title: "{1,2,{\"ru\",\"Организация / Контрагент\"},\
                        {\"en\",\"Организация / Контрагент\"}}",
                type_pattern: "{\"Pattern\",{\"S\"}}",
                ..NativeFormAttributeColumn::default()
            }),
            "{5,4,0,\"ОрганизацияКонтрагентСтрокой\",{1,2,{\"ru\",\"Организация / Контрагент\"},\
             {\"en\",\"Организация / Контрагент\"}},{\"Pattern\",{\"S\"}},\
             {0,{0,{\"B\",1},0}},{0,{0,{\"B\",1},0}},{0,0},0}"
        );
        assert_eq!(
            format_form_attribute_column(&NativeFormAttributeColumn {
                id: "5",
                name: "Характеристика",
                title: "{1,2,{\"ru\",\"Характеристика\"},{\"en\",\"Variant\"}}",
                type_pattern: "{\"Pattern\",{\"#\",89c162bd-68ef-4693-a4a8-19238fa23c62}}",
                functional_options: "{0,1,6c8550bb-f6df-4d47-bbea-cfbc7c362006}",
                ..NativeFormAttributeColumn::default()
            }),
            "{5,5,0,\"Характеристика\",{1,2,{\"ru\",\"Характеристика\"},{\"en\",\"Variant\"}},\
             {\"Pattern\",{\"#\",89c162bd-68ef-4693-a4a8-19238fa23c62}},\
             {0,{0,{\"B\",1},0}},{0,{0,{\"B\",1},0}},\
             {0,1,6c8550bb-f6df-4d47-bbea-cfbc7c362006},0}"
        );
        // A column that says nothing is ten members, and `ShowError` is the
        // only spelling that makes the last one 1.
        let bare = format_form_attribute_column(&NativeFormAttributeColumn::default());
        assert_eq!(top_level_members(&bare).len(), 10);
        assert!(bare.ends_with(",{0,0},0}"));
        assert!(
            format_form_attribute_column(&NativeFormAttributeColumn {
                fill_check: true,
                ..NativeFormAttributeColumn::default()
            })
            .ends_with(",{0,0},1}")
        );

        // A value list's element type, the two ends of the 2 140 measured.
        assert_eq!(
            format_form_value_list_element_type("{\"Pattern\"}"),
            "{0,1,\"ElementType\",{\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d,{\"Pattern\"}}}"
        );
        assert_eq!(
            format_form_value_list_element_type("{\"Pattern\",{\"S\",50,1}}"),
            "{0,1,\"ElementType\",\
             {\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d,{\"Pattern\",{\"S\",50,1}}}}"
        );

        // <CurrentRowUse> is written twice and the two do not agree: Use is 0
        // in both places, saying nothing is 1 and 2, and DontUse is 1 and 1.
        let row_use = |value| {
            let record = format_form_command(&NativeFormCommand {
                current_row_use: value,
                ..NativeFormCommand::default()
            })
            .expect("a command");
            let members = top_level_members(&record);
            (members[13].clone(), members[18].clone())
        };
        assert_eq!(row_use(Some("Use")), ("0".to_string(), "0".to_string()));
        assert_eq!(row_use(Some("DontUse")), ("1".to_string(), "1".to_string()));
        assert_eq!(row_use(None), ("1".to_string(), "2".to_string()));
        assert_eq!(
            format_form_command(&NativeFormCommand {
                current_row_use: Some("Sometimes"),
                ..NativeFormCommand::default()
            }),
            None
        );

        // A parameter is four members, and the key flag is the last.
        assert_eq!(
            format_form_parameter("Отбор", "{\"Pattern\",{\"B\"}}", false),
            "{0,\"Отбор\",{\"Pattern\",{\"B\"}},0}"
        );
        assert_eq!(
            format_form_parameter("Ключ", "{\"Pattern\"}", true),
            "{0,\"Ключ\",{\"Pattern\"},1}"
        );
    }

    /// Splits a record into its top-level members the way the body grammar
    /// does, so a test can say where a child sits.
    fn top_level_members(record: &str) -> Vec<String> {
        let inner = &record[1..record.len() - 1];
        let mut members = Vec::new();
        let mut depth = 0usize;
        let mut start = 0usize;
        let mut in_string = false;
        for (index, ch) in inner.char_indices() {
            match ch {
                '"' => in_string = !in_string,
                _ if in_string => {}
                '{' => depth += 1,
                '}' => depth -= 1,
                ',' if depth == 0 => {
                    members.push(inner[start..index].to_string());
                    start = index + ch.len_utf8();
                }
                _ => {}
            }
        }
        members.push(inner[start..].to_string());
        members
    }

    /// The table record keeps its six children where every ERP УХ table keeps
    /// them: the command bar two members after the context menu, the tooltip
    /// 26 from the end, and the three additions 21, 19 and 17 from the end.
    #[test]
    fn writes_a_table_record_with_its_children_where_the_platform_keeps_them() {
        let context_menu = format_field_context_menu("57", "ОтборКонтекстноеМеню", None, &[]);
        let command_bar = format_empty_auto_command_bar("58", "ОтборКоманднаяПанель");
        let tooltip = format_extended_tooltip("59", "ОтборРасширеннаяПодсказка");
        let record = format_table_item(&NativeTableItem {
            head: "55,{56,02023637-7868-4a5f-8576-835a76e0c9ba},0,2,0,\"Отбор\",{0}",
            context_menu: &context_menu,
            command_bar: &command_bar,
            middle: "0,2,2,1,0,{\"Pattern\"},\"\",\"\",2,2,0",
            extended_tooltip: &tooltip,
            search_string_location: None,
            view_status_location: None,
            search_control_location: None,
            search_string_addition: "{5,{60,x},0}",
            view_status_addition: "{5,{63,x},1}",
            search_control_addition: "{5,{66,x},2}",
            tail: "0,1,0,0,1,0,3,3,0,1,0,0,0,0,0,0",
        });

        let members = top_level_members(&record);
        let context_at = members
            .iter()
            .position(|member| member == &context_menu)
            .expect("the context menu is a member");
        let command_at = members
            .iter()
            .position(|member| member == &command_bar)
            .expect("the command bar is a member");
        let tooltip_at = members
            .iter()
            .position(|member| member == &tooltip)
            .expect("the tooltip is a member");
        let addition_at = members
            .iter()
            .position(|member| member == "{5,{60,x},0}")
            .expect("the search string addition is a member");

        assert_eq!(command_at - context_at, 2);
        assert_eq!(members.len() - tooltip_at, 26);
        assert_eq!(members.len() - addition_at, 21);
        assert_eq!(members[addition_at - 1], "1");
        assert_eq!(members[members.len() - 19], "{5,{63,x},1}");
        assert_eq!(members[members.len() - 17], "{5,{66,x},2}");
        assert_eq!(members.len() - (members.len() - 17) - 1, 16);

        // A table that names none of the three writes 0 for each; one that
        // names them writes the codes the corpus gives.
        assert_eq!(&members[members.len() - 25..members.len() - 22], &["0", "0", "0"]);
        let shown = format_table_item(&NativeTableItem {
            head: "55,{56,02023637-7868-4a5f-8576-835a76e0c9ba},0,2,0,\"Отбор\",{0}",
            context_menu: &context_menu,
            command_bar: &command_bar,
            middle: "0,2,2,1,0,{\"Pattern\"},\"\",\"\",2,2,0",
            extended_tooltip: &tooltip,
            search_string_location: Some("PullFromTop"),
            view_status_location: Some("None"),
            search_control_location: Some("CommandBar"),
            search_string_addition: "{5,{60,x},0}",
            view_status_addition: "{5,{63,x},1}",
            search_control_addition: "{5,{66,x},2}",
            tail: "0,1,0,0,1,0,3,3,0,1,0,0,0,0,0,0",
        });
        let shown = top_level_members(&shown);
        assert_eq!(&shown[shown.len() - 25..shown.len() - 22], &["6", "1", "2"]);
    }

    /// The three additions of the `Отбор` table of an ERP УХ form body, each
    /// exactly as that body stores it.
    #[test]
    fn writes_the_search_additions_the_platform_stores() {
        let search_string = format_search_addition(
            "60",
            0,
            "ОтборСтрокаПоиска",
            "{1,0,2,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0}",
            &format_field_context_menu("61", "ОтборСтрокаПоискаКонтекстноеМеню", None, &[]),
            &format_extended_tooltip("62", "ОтборСтрокаПоискаРасширеннаяПодсказка"),
            "56",
        );
        assert_eq!(
            search_string,
            "{5,{60,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ОтборСтрокаПоиска\",{1,0},{1,0},1,1,0,1,{1,0,2,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0},1,{22,{61,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ОтборСтрокаПоискаКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,{12,{62,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ОтборСтрокаПоискаРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},2,{56,0},0,3,3,0}"
        );

        let search_control = format_search_addition(
            "66",
            2,
            "ОтборУправлениеПоиском",
            "{1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0,2}",
            &format_field_context_menu("67", "ОтборУправлениеПоискомКонтекстноеМеню", None, &[]),
            &format_extended_tooltip("68", "ОтборУправлениеПоискомРасширеннаяПодсказка"),
            "56",
        );
        assert!(search_control.starts_with(
            "{5,{66,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,2,\"ОтборУправлениеПоиском\",{1,0},{1,0},1,1,0,1,"
        ));
        assert!(search_control.ends_with(",2,{56,2},0,3,3,0}"));
    }

    /// Two childless root layouts of ERP УХ form bodies -- one that names a
    /// title and no event, one that names an event and no title -- exactly as
    /// those bodies store them.
    #[test]
    fn writes_the_root_layouts_the_platform_stores() {
        let bar = format_empty_auto_command_bar("-1", "ФормаКоманднаяПанель");
        let scrolling_tail = format_root_tail(&NativeRootTail {
            vertical_scroll: Some("useIfNecessary"),
            ..NativeRootTail::default()
        })
        .unwrap();

        assert_eq!(
            format_root_layout(&NativeRootLayout {
                head: &format_root_head(&NativeRootHead {
                    auto_title: false,
                    title: "{1,2,{\"ru\",\"Заявление на подключение\"},{\"en\",\"Заявление на подключение\"}}",
                    ..NativeRootHead::default()
                })
                .expect("a head"),
                properties: &[],
                events: "{0,1,0}",
                command_set: "{0}",
                command_bar: &bar,
                children: &[],
                tail: &scrolling_tail,
            }),
            "{50,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,0,{1,2,{\"ru\",\"Заявление на подключение\"},{\"en\",\"Заявление на подключение\"}},0,0,1,1,1,0,1,0,{0,1,0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"ФормаКоманднаяПанель\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0},0,\"\",\"\",0,1,\"\",2,0,0,0,0,0,3,3,0,0,2,100,1,1,0,0,0,{50,0},1}"
        );

        assert_eq!(
            format_root_layout(&NativeRootLayout {
                head: &format_root_head(&NativeRootHead {
                    command_bar_location: Some("None"),
                    ..NativeRootHead::default()
                })
                .expect("a head"),
                properties: &[],
                events: "{1,3ccc650e-f631-4cae-8e33-3eaac610b5f9,\"ПриОткрытии\",1,0,3ccc650e-f631-4cae-8e33-3eaac610b5f9,0,1}",
                command_set: "{0}",
                command_bar: &bar,
                children: &[],
                tail: &scrolling_tail,
            }),
            "{50,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{1,3ccc650e-f631-4cae-8e33-3eaac610b5f9,\"ПриОткрытии\",1,0,3ccc650e-f631-4cae-8e33-3eaac610b5f9,0,1},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"ФормаКоманднаяПанель\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0},0,\"\",\"\",0,1,\"\",2,0,0,0,0,0,3,3,0,0,2,100,1,1,0,0,0,{50,0},1}"
        );
    }

    /// A root writes its children the way a group does: the count, then a
    /// `(kind uuid, record)` pair each. The kind uuid says what the record is,
    /// one per wrapper with no overlap.
    #[test]
    fn writes_a_root_with_its_children() {
        assert_eq!(child_kind_uuid(37), Some("77ffcc29-7f2d-4223-b22f-19666e7250ba"));
        assert_eq!(child_kind_uuid(22), Some("cd5394d0-7dda-4b56-8927-93ccbe967a01"));
        assert_eq!(child_kind_uuid(31), Some("a9f3b1ac-f51b-431e-b102-55a69acdecad"));
        assert_eq!(child_kind_uuid(12), Some("3d3cb80c-508b-41fa-8a18-680cdf5f1712"));
        assert_eq!(child_kind_uuid(55), Some("143c00f7-a42d-4cd7-9189-88e4467dc768"));
        assert_eq!(child_kind_uuid(5), Some("c5259a1d-518a-4afd-b98d-0176027e4feb"));
        assert_eq!(child_kind_uuid(50), None);

        let bar = format_empty_auto_command_bar("-1", "FormCommandBar");
        let field = format_field_item(&NativeFieldItem {
            id: "1",
            kind: 2,
            name: "A",
            data_path: "{1,{2}}",
            payload: &format_plain_field_payload(true),
            context_menu: &format_field_context_menu("2", "AM", None, &[]),
            extended_tooltip: &format_extended_tooltip("3", "AT"),
            ..NativeFieldItem::default()
        })
        .expect("a field record");
        let root = format_root_layout(&NativeRootLayout {
            head: &format_root_head(&NativeRootHead {
                command_bar_location: Some("None"),
                ..NativeRootHead::default()
            })
            .expect("a head"),
            properties: &[],
            events: "{0,1,0}",
            command_set: "{0}",
            command_bar: &bar,
            children: &[(child_kind_uuid(37).expect("a field has a kind uuid"), field.clone())],
            tail: &format_root_tail(&NativeRootTail::default()).expect("a default tail"),
        });

        assert!(root.contains(&format!(
            ",1,77ffcc29-7f2d-4223-b22f-19666e7250ba,{field},\"\",\"\",0,1,"
        )));
        assert!(root.ends_with(",0,0,0,{50,0},1}"));
    }

    /// The slots of the input payload the partition test named over all
    /// 69 243 of them -- twenty-four of what the writer used to hold constant.
    #[test]
    fn writes_the_input_slots_the_partition_test_named() {
        let spoken = format_input_payload(&NativeInputPayload {
            wrap: false,
            password_mode: Some(true),
            multi_line: Some(true),
            clear_button: Some(true),
            open_button: Some(false),
            list_choice_mode: true,
            quick_choice: Some(false),
            choice_folders_and_items: Some("Items"),
            choose_type: false,
            incomplete_choice_mode: Some("OnActivate"),
            text_edit: false,
            edit_text_update: Some("Always"),
            choice_button_representation: Some("ShowInInputField"),
            drop_list_button: Some(true),
            choice_history_on_input: Some("DontUse"),
            auto_max_height: false,
            max_height: "4",
            height_control_variant: Some("UseContentHeight"),
            ..NativeInputPayload::plain()
        })
        .expect("an input payload");
        // Wrap, the password and multi-line tri-states, then the buttons.
        assert!(spoken.starts_with("{36,{3,0},0,0,2,2,0,1,1,2,2,2,2,1,2,0,"));
        // The choice mode, the folders-and-items code and the two that follow
        // the formats.
        assert!(spoken.contains(",1,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,0,0,"));
        assert!(spoken.contains(",{1,0},{1,0},2,0,1,{\"Pattern\"},1,"));
        // The text edit flag, the update mode, and the tail's height trio.
        assert!(spoken.contains(",0,{3,0,0},3,{1,0},2,3,1,1,1,0,0,0,4,2,"));

        // A spelling the corpus never showed is refused.
        assert_eq!(
            format_input_payload(&NativeInputPayload {
                edit_text_update: Some("Sometimes"),
                ..NativeInputPayload::plain()
            }),
            None
        );
    }

    /// The four slots of the label payload the partition test named, which
    /// were constants or a guess before: <VerticalStretch>, <MarkNegatives>,
    /// <Hiperlink> and the height's pair of auto-max slots.
    #[test]
    fn writes_the_label_slots_the_partition_test_named() {
        let quiet = format_label_payload(&NativeLabelPayload::plain(false));
        // A label that names none of them carries 2, 2 and 0, and the height
        // pair carries 1 and 0.
        assert!(quiet.starts_with("{11,0,0,2,2,2,{1,0},0,"));
        assert!(quiet.ends_with(",1,0,0,1,0}"));

        let spoken = format_label_payload(&NativeLabelPayload {
            vertical_stretch: Some(true),
            mark_negatives: Some(true),
            hyperlink: true,
            auto_max_height: false,
            max_height: "4",
            ..NativeLabelPayload::plain(false)
        });
        assert!(spoken.starts_with("{11,0,0,2,1,1,{1,0},1,"));
        assert!(spoken.ends_with(",1,0,0,0,4}"));
    }

    /// Four label payloads of ERP УХ form bodies, each carrying a different
    /// set of the five properties the mapping claims, exactly as stored.
    #[test]
    fn writes_the_label_payloads_the_platform_stores() {
        // A label that names only a height: the height lands in slot 2.
        assert_eq!(
            format_label_payload(&NativeLabelPayload { height: "2", ..NativeLabelPayload::plain(false) }),
            "{11,0,2,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0}"
        );
        // Width and height together.
        assert_eq!(
            format_label_payload(&NativeLabelPayload {
                width: "16",
                height: "1",
                ..NativeLabelPayload::plain(false)
            }),
            "{11,16,1,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0}"
        );
        // `HorizontalStretch` is three-valued: false, true, and not named.
        assert!(format_label_payload(&NativeLabelPayload {
            horizontal_stretch: Some(false),
            ..NativeLabelPayload::plain(false)
        })
        .starts_with("{11,0,0,0,"));
        assert!(format_label_payload(&NativeLabelPayload {
            horizontal_stretch: Some(true),
            ..NativeLabelPayload::plain(false)
        })
        .starts_with("{11,0,0,1,"));
        assert!(format_label_payload(&NativeLabelPayload::plain(false)).starts_with("{11,0,0,2,"));
        // A maximum width switches the auto flag off and lands in slot 16.
        assert!(format_label_payload(&NativeLabelPayload {
            auto_max_width: false,
            max_width: "15",
            ..NativeLabelPayload::plain(false)
        })
        .ends_with(",0,15,0,1,0}"));
    }

    /// The payload 24 361 input fields of the corpus carry unchanged, and the
    /// three slots a form most often moves.
    #[test]
    fn writes_the_input_payloads_the_platform_stores() {
        assert_eq!(
            format_input_payload(&NativeInputPayload::plain()).as_deref(),
            Some("{36,{3,0},0,0,2,2,1,2,2,2,2,2,2,2,2,2,{\"U\"},{\"U\"},\"\",0,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,2,3,00000000-0000-0000-0000-000000000000,{5006,0},{0,0},2,{1,0},{1,0},2,1,0,{\"Pattern\"},1,{0,1,0},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},1,{3,0,0},0,{1,0},2,0,2,0,1,0,0,1,0,0,0,0,0,0,0,0,0,{0},0,{5007,0},0}")
        );
        assert!(format_input_payload(&NativeInputPayload {
            width: "25",
            ..NativeInputPayload::plain()
        })
        .expect("an input payload")
        .starts_with("{36,{3,0},25,0,2,2,"));
        assert!(format_input_payload(&NativeInputPayload {
            auto_max_width: false,
            max_width: "28",
            ..NativeInputPayload::plain()
        })
        .expect("an input payload")
        .contains(",2,0,2,0,0,28,0,1,0,"));
        assert!(format_input_payload(&NativeInputPayload {
            mask: "999-999-999 99",
            ..NativeInputPayload::plain()
        })
        .expect("an input payload")
        .contains(",{\"U\"},\"999-999-999 99\",0,"));
    }

    /// The payload of `ГруппаШапка`, a usual group of an ERP УХ form body,
    /// exactly as that body stores it, and the defaults a group that names
    /// nothing carries.
    #[test]
    fn writes_the_usual_group_payloads_the_platform_stores() {
        // `ГруппаШапка`, a usual group of an ERP УХ form body, exactly as that
        // body stores it.
        assert_eq!(
            format_usual_group_payload(&NativeUsualGroupPayload {
                behavior: Some("Usual"),
                group: Some("Horizontal"),
                representation: Some("None"),
                ..NativeUsualGroupPayload::plain()
            })
            .expect("a usual group payload"),
            "{29,1,0,0,1,{0},{1,0},{\"Pattern\"},\"\",{3,4,{0}},0,0,0,1,{1,0},0,0,3,3,2,0,1,1,{3,4,{0}},0,2,0,1,0}"
        );
        // A group that names no arrangement carries 1 in the first slot and 2
        // in both of the others, and one that names no behaviour carries 1, 0
        // and 3. A group that names neither <ShowTitle> nor <United> carries 1
        // in slot 4 and slot 21.
        let plain =
            format_usual_group_payload(&NativeUsualGroupPayload::plain()).expect("a payload");
        assert!(plain.starts_with("{29,1,0,2,1,"));
        assert!(plain.ends_with(",1,2,{3,4,{0}},0,2,0,2,3}"));
        let quiet = format_usual_group_payload(&NativeUsualGroupPayload {
            show_title: false,
            united: false,
            ..NativeUsualGroupPayload::plain()
        })
        .expect("a payload");
        assert!(quiet.starts_with("{29,1,0,2,0,"));
        assert!(quiet.contains(",3,3,2,0,0,2,"));
        let vertical = format_usual_group_payload(&NativeUsualGroupPayload {
            group: Some("Vertical"),
            ..NativeUsualGroupPayload::plain()
        })
        .expect("a payload");
        assert!(vertical.starts_with("{29,0,0,2,1,"));
        assert!(vertical.ends_with(",0,3}"));
        // A spelling the corpus never stores is refused, not defaulted.
        assert!(
            format_usual_group_payload(&NativeUsualGroupPayload {
                group: Some("HorizontalIfPossible"),
                ..NativeUsualGroupPayload::plain()
            })
            .is_none()
        );
    }

    /// The check-box payload 9 906 records carry unchanged, the radio-button
    /// payload, and the slots that carry each field's type.
    #[test]
    fn writes_the_check_box_and_radio_payloads_the_platform_stores() {
        assert_eq!(
            format_check_box_payload(&NativeCheckBoxPayload::plain()).expect("a payload"),
            "{11,0,{3,4,{0}},{3,4,{0}},0,{1,0},{3,4,{0}},{7,3,0,1,100},0,0,0,2,0}"
        );
        // `Auto` and `Switcher` share the first slot and differ in the last.
        assert!(
            format_check_box_payload(&NativeCheckBoxPayload {
                check_box_type: Some("Switcher"),
                ..NativeCheckBoxPayload::plain()
            })
            .expect("a payload")
            .ends_with(",0,0,0,2,3}")
        );
        assert!(
            format_check_box_payload(&NativeCheckBoxPayload {
                check_box_type: Some("Tumbler"),
                ..NativeCheckBoxPayload::plain()
            })
            .expect("a payload")
            .contains("{3,4,{0}},2,{1,0},")
        );
        // `<ThreeState>` and the three sizes, each in its own slot.
        assert!(
            format_check_box_payload(&NativeCheckBoxPayload {
                three_state: true,
                item_title_height: "1",
                item_width: "19",
                item_height: "1",
                equal_items_width: Some("false"),
                ..NativeCheckBoxPayload::plain()
            })
            .expect("a payload")
            .ends_with(",1,19,1,0,0}")
        );

        assert_eq!(
            format_radio_button_payload(&NativeRadioButtonPayload::plain()).expect("a payload"),
            "{8,{3,0},0,{3,4,{0}},{7,3,0,1,100},{3,4,{0}},0,0,{3,4,{0}},0,0,2}"
        );
        assert!(
            format_radio_button_payload(&NativeRadioButtonPayload {
                columns: "3",
                radio_button_type: Some("Tumbler"),
                ..NativeRadioButtonPayload::plain()
            })
            .expect("a payload")
            .starts_with("{8,{3,0},3,{3,4,{0}},{7,3,0,1,100},{3,4,{0}},0,2,")
        );
        // A spelling the corpus never stores is refused, not defaulted.
        assert!(
            format_radio_button_payload(&NativeRadioButtonPayload {
                radio_button_type: Some("Switcher"),
                ..NativeRadioButtonPayload::plain()
            })
            .is_none()
        );
    }

    /// The default payload of each of the six group kinds the corpus closed
    /// in one pass, exactly as those bodies store them.
    #[test]
    fn writes_the_remaining_group_payloads_the_platform_stores() {
        assert_eq!(
            format_button_group_payload("{0}", None).expect("a payload"),
            "{2,{0},2,0}"
        );
        assert_eq!(
            format_button_group_payload("{0}", Some("Compact")).expect("a payload"),
            "{2,{0},2,2}"
        );
        assert_eq!(
            format_command_bar_payload(None, "{0}").expect("a payload"),
            "{1,0,{0}}"
        );
        assert_eq!(
            format_command_bar_payload(Some("Auto"), "{0}").expect("a payload"),
            "{1,3,{0}}"
        );
        assert_eq!(
            format_pages_payload(None, "{0,1,0}", "0").expect("a payload"),
            "{4,1,{0,1,0},2,0,6}"
        );
        assert_eq!(
            format_pages_payload(Some("None"), "{0,1,0}", "0").expect("a payload"),
            "{4,0,{0,1,0},2,0,0}"
        );
        assert_eq!(
            format_popup_payload(&NativePopupPayload::plain()).expect("a payload"),
            "{7,{4,0,{0},\"\",-1,-1,1,0,\"\"},{0},2,3,0,0,{3,4,{0}},{3,4,{0}}}"
        );
        assert_eq!(
            format_popup_payload(&NativePopupPayload {
                representation: Some("Picture"),
                shape: Some("Oval"),
                shape_representation: Some("None"),
                ..NativePopupPayload::plain()
            })
            .expect("a payload"),
            "{7,{4,0,{0},\"\",-1,-1,1,0,\"\"},{0},2,1,2,3,{3,4,{0}},{3,4,{0}}}"
        );
        assert_eq!(
            format_column_group_payload(&NativeColumnGroupPayload::plain()).expect("a payload"),
            "{2,1,1,0,3,{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{0},{\"Pattern\"},\"\",{1,0},0}"
        );
        assert_eq!(
            format_page_payload(&NativePagePayload::plain()).expect("a payload"),
            "{18,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,{0},{1,0},1,{\"Pattern\"},\"\",{3,4,{0}},0,0,3,3,0,0,0,0,{3,4,{0}},{7,3,0,1,100}}"
        );
        // `AlwaysHorizontal` and `Horizontal` differ in the second of the two
        // slots that carry the arrangement.
        assert!(
            format_page_payload(&NativePagePayload {
                group: Some("AlwaysHorizontal"),
                ..NativePagePayload::plain()
            })
            .expect("a payload")
            .contains(",0,0,1,3,{3,4,{0}},")
        );
        assert!(
            format_page_payload(&NativePagePayload {
                group: Some("Horizontal"),
                ..NativePagePayload::plain()
            })
            .expect("a payload")
            .contains(",0,0,1,1,{3,4,{0}},")
        );
        // A spelling the corpus never stores is refused, not defaulted.
        assert!(format_button_group_payload("{0}", Some("Auto")).is_none());
    }

    /// The document and picture payloads, exactly as ERP УХ bodies store
    /// them.
    #[test]
    fn writes_the_document_and_picture_payloads_the_platform_stores() {
        assert_eq!(
            format_document_payload(3, "50", "10", "{3,4,{0}},0,{0,1,0},1,0,0,1,0,1,1"),
            "{3,50,10,{3,4,{0}},0,{0,1,0},1,0,0,1,0,1,1}"
        );
        assert_eq!(
            format_document_payload(5, "50", "10", "1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},1,0,0,1,0,{0,1,0}"),
            "{5,50,10,1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},1,0,0,1,0,{0,1,0}}"
        );
        assert_eq!(
            format_document_payload(1, "50", "10", "1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0,1,0"),
            "{1,50,10,1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0,1,0}"
        );

        // The picture size spellings the second round added.
        let picture = |size: Option<&str>| {
            format_picture_payload(&NativePicturePayload {
                width: "2",
                picture_size: size,
                file_drag_mode: Some("AsFile"),
                ..NativePicturePayload::default()
            })
            .expect("a picture payload")
        };
        assert!(picture(None).contains("-1,-1,1,0,\"\"},0,0,0,{1,0},"));
        assert!(picture(Some("ByFontSize")).contains("-1,-1,1,0,\"\"},7,0,0,{1,0},"));
        assert!(picture(Some("AutoSizeIgnoreScale")).contains("-1,-1,1,0,\"\"},6,0,0,{1,0},"));
        // The six slots the partition test named: the two stretches read
        // two-valued, the drag mode is 1 when the field names none.
        let spoken = format_picture_payload(&NativePicturePayload {
            horizontal_stretch: false,
            vertical_stretch: false,
            hyperlink: true,
            auto_max_width: false,
            max_width: "6",
            auto_max_height: false,
            max_height: "3",
            ..NativePicturePayload::default()
        })
        .expect("a picture payload");
        assert!(spoken.starts_with("{10,0,0,0,0,"));
        assert!(spoken.contains(",0,0,1,{1,0},"));
        assert!(spoken.ends_with(",{0,1,0},0,6,0,0,3,1,100}"));
        assert_eq!(
            format_picture_payload(&NativePicturePayload {
                picture_size: Some("Tile"),
                ..NativePicturePayload::default()
            }),
            None
        );

        // A spreadsheet field that names neither scroll bar carries 1 in the
        // first slot that holds it and 2 in the second.
        let sheet = |vertical, horizontal| {
            format_spreadsheet_payload(&NativeSpreadsheetPayload {
                vertical_scroll_bar: vertical,
                horizontal_scroll_bar: horizontal,
                ..NativeSpreadsheetPayload::plain()
            })
            .expect("a payload")
        };
        assert_eq!(
            sheet(None, None),
            "{13,50,10,1,1,0,0,1,1,0,0,1,0,0,1,{3,4,{0}},1,1,{0,1,0},0,1,0,0,1,0,0,0,0,2,2,1,2}"
        );
        assert!(sheet(Some("true"), Some("false")).ends_with(",1,0,1,2}"));
        assert!(sheet(Some("true"), Some("false")).contains(",1,1,0,0,1,0,"));
    }

    /// Every colour shape an ERP УХ body stores, with the exact values the
    /// corpus gives.
    #[test]
    fn writes_the_colours_the_platform_stores() {
        let none = |_: &str| None;
        assert_eq!(format_native_color(None, none).as_deref(), Some("{3,4,{0}}"));
        // The three bytes in the other order: blue first.
        assert_eq!(
            format_native_color(Some("#0000FF"), none).as_deref(),
            Some("{3,0,{16711680}}")
        );
        assert_eq!(
            format_native_color(Some("#800000"), none).as_deref(),
            Some("{3,0,{128}}")
        );
        assert_eq!(
            format_native_color(Some("#FFFFFF"), none).as_deref(),
            Some("{3,0,{16777215}}")
        );
        assert_eq!(
            format_native_color(Some("style:ToolTipBackColor"), none).as_deref(),
            Some("{3,3,{-23}}")
        );
        assert_eq!(
            format_native_color(Some("web:MistyRose"), none).as_deref(),
            Some("{3,2,{98}}")
        );
        assert_eq!(
            format_native_color(Some("style:ГиперссылкаЦвет"), |name| {
                (name == "ГиперссылкаЦвет")
                    .then(|| "757b547b-b79c-459a-a64a-eef19a09a38f".to_string())
            })
            .as_deref(),
            Some("{3,3,{0,757b547b-b79c-459a-a64a-eef19a09a38f}}")
        );
        // A spelling the writer cannot place is refused, not defaulted.
        assert_eq!(format_native_color(Some("web:Chartreuse"), none), None);
        assert_eq!(format_native_color(Some("style:Неизвестный"), none), None);
        assert_eq!(format_native_color(Some("#12345"), none), None);
    }

    /// The root record's head, with the bytes ERP УХ stores.
    #[test]
    fn writes_the_root_heads_the_platform_stores() {
        // The smallest form of the corpus turns AutoTitle off and names
        // nothing else, so every other member takes its absent value.
        assert_eq!(
            format_root_head(&NativeRootHead {
                auto_title: false,
                ..NativeRootHead::default()
            })
            .as_deref(),
            Some("50,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,0,{1,0},0,0,1,1,1,0,1")
        );

        // A form that opens locking its owner, sizes itself, and puts its
        // command bar at the bottom.
        assert_eq!(
            format_root_head(&NativeRootHead {
                window_opening_mode: Some("LockOwnerWindow"),
                width: Some("45"),
                height: Some("30"),
                command_bar_location: Some("Bottom"),
                ..NativeRootHead::default()
            })
            .as_deref(),
            Some("50,0,1,45,30,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,3")
        );

        // Every switch the head carries, off, and the three settings it reads
        // as codes.
        assert_eq!(
            format_root_head(&NativeRootHead {
                enter_key_behavior: Some("DefaultButton"),
                save_data_in_settings: Some("UseList"),
                auto_save_data_in_settings: Some("Use"),
                group: Some("Horizontal"),
                child_items_width: Some("LeftNarrowest"),
                auto_fill_check: false,
                customizable: false,
                enabled: false,
                command_bar_location: Some("None"),
                ..NativeRootHead::default()
            })
            .as_deref(),
            Some("50,0,0,0,0,0,1,1,00000000-0000-0000-0000-000000000000,1,{1,0},1,5,0,0,0,0,0")
        );

        // A form that names a settings storage carries that object's uuid,
        // resolved by the caller, in member 8.
        assert!(
            format_root_head(&NativeRootHead {
                settings_storage: Some("0f7a4e33-1c7d-4c5f-9a4b-3f0f1b8a2e61"),
                ..NativeRootHead::default()
            })
            .expect("a head")
            .contains(",0f7a4e33-1c7d-4c5f-9a4b-3f0f1b8a2e61,")
        );

        // So is a spelling the corpus never showed.
        assert_eq!(
            format_root_head(&NativeRootHead {
                command_bar_location: Some("Nowhere"),
                ..NativeRootHead::default()
            }),
            None
        );
    }

    /// The root record's tail, with the bytes ERP УХ stores.
    #[test]
    fn writes_the_root_tails_the_platform_stores() {
        // The smallest form of the corpus: it turns AutoURL off and scrolls
        // vertically if necessary, and names nothing else.
        assert_eq!(
            format_root_tail(&NativeRootTail {
                auto_url: false,
                vertical_scroll: Some("useIfNecessary"),
                ..NativeRootTail::default()
            })
            .as_deref(),
            Some("\"\",\"\",0,0,\"\",2,0,0,0,0,0,3,3,0,0,2,100,1,1,0,0,0,{50,0},1")
        );

        // The same form with a navigator: the flag turns on and the record
        // follows it, which is the one member that makes a tail 25 long.
        let navigator = format_root_tail(&NativeRootTail {
            auto_url: false,
            vertical_scroll: Some("useIfNecessary"),
            navigator: Some("{22,{0},0}"),
            ..NativeRootTail::default()
        })
        .expect("a navigator tail");
        assert!(navigator.starts_with("\"\",\"\",1,{22,{0},0},0,"));

        // useWithoutStretch is not the same code in both places it is read.
        let stretch = format_root_tail(&NativeRootTail {
            vertical_scroll: Some("useWithoutStretch"),
            ..NativeRootTail::default()
        })
        .expect("a stretch tail");
        assert!(stretch.starts_with("\"\",\"\",0,1,\"\",0,"));
        assert!(stretch.contains(",3,3,0,0,3,100,"));

        // AlwaysHorizontal is not the same code in both places either.
        let group = format_root_tail(&NativeRootTail {
            group: Some("AlwaysHorizontal"),
            ..NativeRootTail::default()
        })
        .expect("a grouped tail");
        assert!(group.contains(",3,3,0,1,0,100,"));
        assert!(group.ends_with(",0,0,3,{50,0},1"));

        // Every switch the tail carries, off.
        let off = format_root_tail(&NativeRootTail {
            auto_url: false,
            show_title: false,
            show_close_button: false,
            save_window_settings: false,
            ..NativeRootTail::default()
        })
        .expect("a tail with its switches off");
        assert!(off.contains(",100,0,0,"));
        assert!(off.ends_with(",{50,0},0"));

        // A spelling the corpus never showed is refused rather than defaulted.
        assert_eq!(
            format_root_tail(&NativeRootTail {
                group: Some("InCell"),
                ..NativeRootTail::default()
            }),
            None
        );
        assert_eq!(
            format_root_tail(&NativeRootTail {
                vertical_align: Some("Stretch"),
                ..NativeRootTail::default()
            }),
            None
        );
    }

    /// Every event-binding shape the corpus stores, with the exact bytes.
    #[test]
    fn writes_the_event_bindings_the_platform_stores() {
        let event = |name, handler| NativeEvent { name, handler };

        // An owner with no events.
        assert_eq!(
            format_native_events("InputField", "", &[]).as_deref(),
            Some("{0,1,0}")
        );

        // One event of an input field.
        assert_eq!(
            format_native_events("InputField", "", &[event("OnChange", "ОтборСчетПриИзменении")])
                .as_deref(),
            Some("{1,fe115cc8-9e33-4684-a166-bd5136fe7a9f,\"ОтборСчетПриИзменении\",1,0,fe115cc8-9e33-4684-a166-bd5136fe7a9f,0,1}")
        );

        // Two events of a table: both pairs first, then both tails.
        assert_eq!(
            format_native_events(
                "Table",
                "",
                &[
                    event("Selection", "СписокВыбор"),
                    event(
                        "BeforeLoadUserSettingsAtServer",
                        "СписокПередЗагрузкойПользовательскихНастроекНаСервере"
                    ),
                ]
            )
            .as_deref(),
            Some("{2,1282f000-23b6-4887-87f4-9e8e79db3d32,\"СписокВыбор\",c41e7b98-098c-433e-8ac3-56ec2a2c49e2,\"СписокПередЗагрузкойПользовательскихНастроекНаСервере\",1,0,1282f000-23b6-4887-87f4-9e8e79db3d32,0,1,c41e7b98-098c-433e-8ac3-56ec2a2c49e2,0,1}")
        );

        // One event an extension gave a second handler: one pair, one tail, and
        // the extension's handler inside that tail.
        assert_eq!(
            format_native_events(
                "InputField",
                "",
                &[
                    event("OnChange", "НаборДанныхБазыРаспределенияПриИзменении"),
                    event("OnChange", "Расш1_НаборДанныхБазыРаспределенияПриИзмененииПосле"),
                ]
            )
            .as_deref(),
            Some("{1,fe115cc8-9e33-4684-a166-bd5136fe7a9f,\"НаборДанныхБазыРаспределенияПриИзменении\",1,0,fe115cc8-9e33-4684-a166-bd5136fe7a9f,0,2,\"Расш1_НаборДанныхБазыРаспределенияПриИзмененииПосле\",1}")
        );

        // A form-level event whose uuid the main attribute decides: a document
        // form writes one, every other form that declares it the other, and a
        // class the corpus never showed declaring it is refused.
        let before_write = |class| {
            format_native_events("Form", class, &[event("BeforeWrite", "ПередЗаписью")])
        };
        assert!(before_write("cfg:DocumentObject")
            .unwrap()
            .contains("8a5894c9-d2ff-4c1d-b433-89cc352bbfbc"));
        assert!(before_write("cfg:CatalogObject")
            .unwrap()
            .contains("9cc34712-da5f-4faa-a653-343d2085fbe8"));
        assert_eq!(before_write(""), None);

        // An event the platform could not spell carries its uuid as its name.
        assert_eq!(
            format_native_events("Form", "", &[event("b3c10170-c5ff-4cba-b537-679e1c872b45", "Обработчик")]).as_deref(),
            Some("{1,b3c10170-c5ff-4cba-b537-679e1c872b45,\"Обработчик\",1,0,b3c10170-c5ff-4cba-b537-679e1c872b45,0,1}")
        );

        // A name no kind declares is refused, and so is one another kind
        // declares: a table's OnActivateRow is not an input field's.
        assert_eq!(
            format_native_events("InputField", "", &[event("NoSuchEvent", "X")]),
            None
        );
        assert_eq!(
            format_native_events("InputField", "", &[event("OnActivateRow", "X")]),
            None
        );
    }

    /// The tuple, over the kinds and the attribute combinations the corpora
    /// spell, and the refusal of the one shape they do not decide.
    #[test]
    fn writes_the_fonts_the_platform_stores() {
        let none = |_: &str| None;
        let font = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect::<BTreeMap<_, _>>()
        };
        let style = |name: &str| {
            (name == "ШрифтНовостей").then(|| "db43d980-40e2-4f50-b17b-fac3bd9e2773".to_string())
        };

        assert_eq!(
            format_native_font(&BTreeMap::new(), none).as_deref(),
            Some("{7,3,0,1,100}")
        );
        // A style item the configuration declares, and one of the five
        // platform fonts, which no `StyleItems\<name>.xml` carries.
        assert_eq!(
            format_native_font(
                &font(&[("kind", "StyleItem"), ("ref", "style:ШрифтНовостей")]),
                style
            )
            .as_deref(),
            Some("{7,2,0,{0,db43d980-40e2-4f50-b17b-fac3bd9e2773},1,100}")
        );
        assert_eq!(
            format_native_font(
                &font(&[("kind", "StyleItem"), ("ref", "style:NormalTextFont")]),
                none
            )
            .as_deref(),
            Some("{7,2,0,{-31},1,100}")
        );
        // The mask names the attributes that are set and the values follow in
        // an order of their own: height, bold, italic, underline, strikeout,
        // faceName. Height is tenths of a point and bold is 700 or 400.
        assert_eq!(
            format_native_font(
                &font(&[
                    ("kind", "StyleItem"),
                    ("ref", "style:NormalTextFont"),
                    ("bold", "true"),
                ]),
                none
            )
            .as_deref(),
            Some("{7,2,4,{-31},700,1,100}")
        );
        assert_eq!(
            format_native_font(
                &font(&[
                    ("kind", "WindowsFont"),
                    ("ref", "sys:DefaultGUIFont"),
                    ("faceName", "Arial"),
                    ("height", "8"),
                    ("italic", "false"),
                ]),
                none
            )
            .as_deref(),
            Some("{7,1,11,{0},80,0,\"Arial\",1,100}")
        );
        assert_eq!(
            format_native_font(&font(&[("kind", "AutoFont"), ("scale", "120")]), none).as_deref(),
            Some("{7,3,512,1,120}")
        );
        // `Absolute` is a fixed nineteen-member LOGFONT, and every one of the
        // 1 321 in the corpora spells all seven attributes. The five records
        // whose LOGFONT block differs export to the same element, so the
        // majority shape is written rather than refused.
        assert_eq!(
            format_native_font(
                &font(&[
                    ("kind", "Absolute"),
                    ("faceName", "Arial"),
                    ("height", "10"),
                    ("bold", "true"),
                    ("italic", "false"),
                    ("underline", "false"),
                    ("strikeout", "false"),
                    ("scale", "100"),
                ]),
                none
            )
            .as_deref(),
            Some("{7,0,575,100,0,0,0,700,0,0,0,0,0,0,0,0,\"Arial\",1,100}")
        );
        // One that omits an attribute is a spelling the corpus never showed,
        // and the reader would add it back on the way out, so it refuses.
        assert_eq!(
            format_native_font(&font(&[("kind", "Absolute"), ("height", "8")]), none),
            None
        );
        assert_eq!(
            format_native_font(
                &font(&[("kind", "StyleItem"), ("ref", "style:НетТакого")]),
                none
            ),
            None
        );
        assert_eq!(
            format_native_font(
                &font(&[("kind", "AutoFont"), ("charSet", "204")]),
                none
            ),
            None
        );
    }

    /// A name that carries a quote is escaped the way every other 1C string in
    /// a body is, rather than being written raw.
    #[test]
    fn escapes_a_quoted_name() {
        assert!(format_extended_tooltip("1", "a\"b").contains("\"a\"\"b\""));
    }

    // -----------------------------------------------------------------------
    // `<DataPath>`
    //
    // Every expectation below is a stored member, read out of an ERP УХ body
    // together with the form and the configuration facts it was computed from.
    // -----------------------------------------------------------------------

    /// The configuration source tree, as a test spells it.
    struct TestConfiguration(BTreeMap<String, Arc<ConfigurationObject>>);

    impl ConfigurationObjects for TestConfiguration {
        fn object(&self, key: &str) -> Option<Arc<ConfigurationObject>> {
            self.0.get(key).cloned()
        }
    }

    impl TestConfiguration {
        fn new(objects: Vec<(&str, ConfigurationObject)>) -> Self {
            Self(
                objects
                    .into_iter()
                    .map(|(key, object)| (key.to_string(), Arc::new(object)))
                    .collect(),
            )
        }
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn attribute(id: &str, types: &[&str]) -> DataPathAttribute {
        DataPathAttribute {
            id: id.to_string(),
            types: strings(types),
            ..DataPathAttribute::default()
        }
    }

    fn field(uuid: &str, tag: &str, types: &[&str]) -> Vec<ConfigurationField> {
        vec![ConfigurationField {
            uuid: uuid.to_string(),
            tag: tag.to_string(),
            types: strings(types),
        }]
    }

    fn form_with(attributes: Vec<(&str, DataPathAttribute)>) -> DataPathForm {
        DataPathForm {
            attributes: attributes
                .into_iter()
                .map(|(name, entry)| (name.to_string(), entry))
                .collect(),
            items: BTreeMap::new(),
        }
    }

    /// `Catalogs/КлассификаторПолномочийМЧД003/Forms/ФормаПодбораСоставныхПолномочий`
    /// binds a field to `Объект`, whose `<Attribute id="13">` is the whole of
    /// the answer.
    #[test]
    fn writes_a_dot_free_data_path_as_the_attribute_id() {
        let form = form_with(vec![("Объект", attribute("13", &["xs:string"]))]);
        assert_eq!(
            resolve_form_data_path(&form, None, "Объект").as_deref(),
            Some("{1,{13}}")
        );
        // A leading `~` is not part of the name.
        assert_eq!(
            resolve_form_data_path(&form, None, "~Объект").as_deref(),
            Some("{1,{13}}")
        );
        // A name the form does not declare refuses.
        assert_eq!(resolve_form_data_path(&form, None, "Запись"), None);
    }

    /// `Documents/ЗаявлениеАбонентаСпецоператораСвязи/Forms/…ФормаДокумента`
    /// binds to `Объект.Получатели.ТипПолучателя`: the tabular section's own
    /// `uuid=` and then the section field's own `uuid=`, neither of them an
    /// ordinal.
    #[test]
    fn writes_a_tabular_section_field_by_its_own_uuid() {
        let form = form_with(vec![(
            "Объект",
            attribute(
                "1",
                &["cfg:DocumentObject.ЗаявлениеАбонентаСпецоператораСвязи"],
            ),
        )]);
        let configuration = TestConfiguration::new(vec![(
            "Document.ЗаявлениеАбонентаСпецоператораСвязи",
            ConfigurationObject {
                uuid: "f87d43ef-51a7-4023-8420-68cb17e482c2".to_string(),
                class: "Document".to_string(),
                owners: Vec::new(),
                fields: BTreeMap::from([(
                    "Получатели".to_string(),
                    field(
                        "1788182c-430d-40f4-bd47-fd59b6eaa3e0",
                        "TabularSection",
                        &[],
                    ),
                )]),
                sections: BTreeMap::from([(
                    "Получатели".to_string(),
                    BTreeMap::from([(
                        "ТипПолучателя".to_string(),
                        field("058ce803-cc68-45f4-a266-3fe076e3e96e", "Attribute", &[]),
                    )]),
                )]),
                types: Vec::new(),
            },
        )]);
        assert_eq!(
            resolve_form_data_path(
                &form,
                Some(&configuration),
                "Объект.Получатели.ТипПолучателя"
            )
            .as_deref(),
            Some(
                "{3,{1},{0,1788182c-430d-40f4-bd47-fd59b6eaa3e0},\
                 {0,058ce803-cc68-45f4-a266-3fe076e3e96e}}"
            )
        );
        // `LineNumber` on the same section is the tabular-section table's −2,
        // not anything the catalog's own table holds.
        assert_eq!(
            resolve_form_data_path(&form, Some(&configuration), "Объект.Получатели.LineNumber")
                .as_deref(),
            Some("{3,{1},{0,1788182c-430d-40f4-bd47-fd59b6eaa3e0},{-2}}")
        );
    }

    /// `Catalogs/АлгоритмыСбораДанныхБухОтчетности/Forms/ФормаЭлемента` stores
    /// `{2,{1},{-3}}` for `Объект.Description`. The number is a platform
    /// constant and differs per class, so the same name on a chart of accounts
    /// is −8; a name the measured table does not know refuses.
    #[test]
    fn writes_a_standard_attribute_as_its_platform_number() {
        let catalog = form_with(vec![(
            "Объект",
            attribute(
                "1",
                &["cfg:CatalogObject.АлгоритмыСбораДанныхБухОтчетности"],
            ),
        )]);
        let configuration = TestConfiguration::new(vec![
            (
                "Catalog.АлгоритмыСбораДанныхБухОтчетности",
                ConfigurationObject {
                    uuid: "47e8bdc9-1704-4e36-bf2d-4123aa2387ca".to_string(),
                    class: "Catalog".to_string(),
                    ..ConfigurationObject::default()
                },
            ),
            (
                "ChartOfAccounts.Хозрасчетный",
                ConfigurationObject {
                    uuid: "1c1d0b1b-0000-4000-8000-000000000001".to_string(),
                    class: "ChartOfAccounts".to_string(),
                    ..ConfigurationObject::default()
                },
            ),
        ]);
        assert_eq!(
            resolve_form_data_path(&catalog, Some(&configuration), "Объект.Description").as_deref(),
            Some("{2,{1},{-3}}")
        );

        let accounts = form_with(vec![(
            "Объект",
            attribute("1", &["cfg:ChartOfAccountsObject.Хозрасчетный"]),
        )]);
        assert_eq!(
            resolve_form_data_path(&accounts, Some(&configuration), "Объект.Description")
                .as_deref(),
            Some("{2,{1},{-8}}")
        );

        // `DeletionMark` is a standard attribute of every catalog, but no form
        // of the corpus binds one, so its number was never measured. Writing
        // a guess would load wrong; refusing is the whole point of the table.
        assert_eq!(
            resolve_form_data_path(&catalog, Some(&configuration), "Объект.DeletionMark"),
            None
        );
    }

    /// `AccumulationRegisters/ПланыПроизводства/Forms/ФормаРедактированияПолуфабриката`
    /// binds to `Items.СписокКорректировок.CurrentData.Спецификация`: three
    /// parts collapse into the item's own pair, and the walk carries on in the
    /// context of the *item's* `<DataPath>`.
    #[test]
    fn collapses_an_items_current_data_head() {
        let form = DataPathForm {
            attributes: BTreeMap::from([(
                "СписокКорректировок".to_string(),
                attribute("1", &["cfg:AccumulationRegisterRecordSet.ПланыПроизводства"]),
            )]),
            items: BTreeMap::from([(
                "СписокКорректировок".to_string(),
                DataPathItem {
                    id: "64".to_string(),
                    data_path: Some("СписокКорректировок".to_string()),
                },
            )]),
        };
        let configuration = TestConfiguration::new(vec![(
            "AccumulationRegister.ПланыПроизводства",
            ConfigurationObject {
                uuid: "8d0ad0b0-0000-4000-8000-000000000002".to_string(),
                class: "AccumulationRegister".to_string(),
                fields: BTreeMap::from([(
                    "Спецификация".to_string(),
                    field("5c4b51c2-0d7a-41b6-b7a0-7437a78c9147", "Dimension", &[]),
                )]),
                ..ConfigurationObject::default()
            },
        )]);
        assert_eq!(
            resolve_form_data_path(
                &form,
                Some(&configuration),
                "Items.СписокКорректировок.CurrentData.Спецификация"
            )
            .as_deref(),
            Some(
                "{2,{64,02023637-7868-4a5f-8576-835a76e0c9ba},\
                 {0,5c4b51c2-0d7a-41b6-b7a0-7437a78c9147}}"
            )
        );
        // The head alone is one segment, `CurrentData` or not.
        assert_eq!(
            resolve_form_data_path(&form, Some(&configuration), "Items.СписокКорректировок")
                .as_deref(),
            Some("{1,{64,02023637-7868-4a5f-8576-835a76e0c9ba}}")
        );
    }

    /// The two things the rule does not place, and refuses rather than guess.
    ///
    /// A dynamic list's query field stores the number the same form's own
    /// `FieldsMap` carries, and no `Form.xml` of the corpus holds one -- 0 of
    /// 12 507 contain the string. An `ExtDimension<N>` of an accounting
    /// register stores a uuid that is in neither the register's file nor its
    /// chart of accounts.
    #[test]
    fn refuses_the_two_numbers_no_source_file_holds() {
        let list = form_with(vec![("Список", attribute("1", &["cfg:DynamicList"]))]);
        assert_eq!(
            resolve_form_data_path(&list, None, "Список.ExtDimensionDr1"),
            None
        );
        assert_eq!(resolve_form_data_path(&list, None, "Список.Организация"), None);
        // The list's own three members are platform constants and do place.
        assert_eq!(
            resolve_form_data_path(&list, None, "Список.Order").as_deref(),
            Some("{2,{1},{-1}}")
        );
        assert_eq!(
            resolve_form_data_path(&list, None, "Список.Filter").as_deref(),
            Some("{2,{1},{-2}}")
        );

        let records = form_with(vec![(
            "ПроводкиСКорреспонденцией",
            attribute("3", &["cfg:AccountingRegisterRecordSet.Международный"]),
        )]);
        let configuration = TestConfiguration::new(vec![(
            "AccountingRegister.Международный",
            ConfigurationObject {
                uuid: "3f1ab7cc-0000-4000-8000-000000000003".to_string(),
                class: "AccountingRegister".to_string(),
                ..ConfigurationObject::default()
            },
        )]);
        // An accounting register's extra dimensions are platform constants,
        // one uuid per side with N-1 in front (rt-paths.md §4.4, 81 of 81).
        assert_eq!(
            resolve_form_data_path(
                &records,
                Some(&configuration),
                "ПроводкиСКорреспонденцией.ExtDimensionDr1"
            )
            .as_deref(),
            Some("{2,{3},{0,1ab44b24-3315-40a9-b495-f1f1227ac205}}")
        );
        // Without a configuration to read, a dotted path refuses too.
        assert_eq!(
            resolve_form_data_path(&records, None, "ПроводкиСКорреспонденцией.Период"),
            None
        );
    }
}
