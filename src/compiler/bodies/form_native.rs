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
    ("AuxiliaryNavigationColor", "-43"),
    ("ButtonBackColor", "-7"),
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
    ("SpecialTextColor", "-16"),
    ("TableFooterBackColor", "-37"),
    ("TableHeaderBackColor", "-35"),
    ("ToolTipBackColor", "-23"),
    ("ToolTipTextColor", "-24"),
];

/// The web colours the corpus names, with the index a body stores.
const WEB_COLOR_CODES: &[(&str, &str)] = &[
    ("FireBrick", "44"),
    ("ForestGreen", "46"),
    ("Gray", "52"),
    ("HoneyDew", "55"),
    ("IndianRed", "57"),
    ("LightGreen", "70"),
    ("LightYellow", "79"),
    ("MistyRose", "98"),
    ("Red", "119"),
    ("WhiteSmoke", "144"),
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
    ("Form", "AccountingRegisterRecordSet", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "AccountingRegisterRecordSet", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "BusinessProcessObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "CatalogObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "CatalogObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "ChartOfAccountsObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "ChartOfCalculationTypesObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "ChartOfCalculationTypesObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "ChartOfCharacteristicTypesObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "ChartOfCharacteristicTypesObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "ConstantsSet", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "DocumentObject", "BeforeWrite", "8a5894c9-d2ff-4c1d-b433-89cc352bbfbc"),
    ("Form", "DocumentObject", "BeforeWriteAtServer", "8f42e083-be92-4102-b1f0-fa58452c1a63"),
    ("Form", "ExchangePlanObject", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "ExchangePlanObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "InformationRegisterRecordManager", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "InformationRegisterRecordManager", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "InformationRegisterRecordSet", "BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("Form", "InformationRegisterRecordSet", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
    ("Form", "TaskObject", "BeforeWriteAtServer", "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6"),
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
    kind: Option<&str>,
    reference: Option<&str>,
    has_other_attributes: bool,
    style_item_uuid: impl FnOnce(&str) -> Option<String>,
) -> Option<String> {
    if kind.is_none() && reference.is_none() {
        return Some("{7,3,0,1,100}".to_string());
    }
    if has_other_attributes || kind != Some("StyleItem") {
        return None;
    }
    let name = reference?.strip_prefix("style:")?;
    // A platform style font is a negative code this writer has not measured.
    if PLATFORM_STYLE_COLOR_CODES
        .iter()
        .any(|(candidate, _)| *candidate == name)
    {
        return None;
    }
    style_item_uuid(name).map(|uuid| format!("{{7,2,0,{{0,{uuid}}},1,100}}"))
}

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
pub(crate) fn format_field_context_menu(id: &str, name: &str) -> String {
    format!(
        "{{22,{{{id},{ns}}},0,0,0,8,{name},{{1,0}},{{1,0}},0,1,0,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{1,1}},0,1,0,0,0,3,3,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
    )
}

/// What a field record needs from the source beyond its own name.
pub(crate) struct NativeFieldItem<'a> {
    pub(crate) id: &'a str,
    /// Which field this is: 1 a label, 2 an input, 3 a check box, 4 a picture,
    /// 5 a radio-button group, 6 a spreadsheet document, 7 an HTML document,
    /// 9 an indicator, 15 a formatted document. Censused over the `{37,…}`
    /// records of the first 800 ERP УХ form bodies: all of them share this one
    /// frame of 59 members -- 60 when the item carries the visibility tuple --
    /// and differ only in this slot, in `after_name`, in the data path and in
    /// the payload.
    pub(crate) kind: u8,
    pub(crate) name: &'a str,
    /// Whether the item carries the common `UserVisible` tuple before its
    /// kind, as 527 of the 2 836 kind-2 records do.
    pub(crate) visible_tuple: bool,
    /// The two members that follow the name -- `1,0` for a label or an input,
    /// `4,0` for a check box, `0,0` for a spreadsheet document.
    pub(crate) after_name: &'a str,
    /// The binding the item's `<DataPath>` resolves to, already formatted --
    /// `{1,{2}}` for a form attribute, `{2,{1},{3}}` for a dynamic-list column.
    pub(crate) data_path: &'a str,
    /// The tuple that carries this kind's own properties: `{11,…}` for a
    /// label, `{36,…}` for an input, `{13,…}` for a spreadsheet document,
    /// `{10,…}` for a picture, and so on -- one structure per kind.
    pub(crate) payload: &'a str,
    pub(crate) context_menu_id: &'a str,
    pub(crate) context_menu_name: &'a str,
    pub(crate) extended_tooltip_id: &'a str,
    pub(crate) extended_tooltip_name: &'a str,
}

/// The `{37,…}` record of a field of any kind that carries only its name, its
/// data path, its kind payload and its two default children.
///
/// Measured against `Documents/Лот/Forms/ВыигранныеЛоты`, whose three label
/// fields and one input field are exactly this shape, and against the census
/// of every field kind in the corpus.
pub(crate) fn format_field_item(item: &NativeFieldItem<'_>) -> String {
    let visibility = if item.visible_tuple {
        "1,{0,{0,{\"B\",1},0}}"
    } else {
        "0"
    };
    format!(
        "{{37,{{{id},{ns}}},0,0,{visibility},{kind},{name},{after_name},{{1,0}},{{1,0}},\
         {data_path},{{0}},1,0,2,0,2,{{1,0}},{{1,0}},1,1,0,3,0,3,1,3,0,\
         {{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},{{0,0,0}},1,\
         {payload},{{0,1,0}},1,\
         {context_menu},1,{{\"Pattern\"}},{{\"Pattern\"}},\"\",\"\",{{0}},0,0,1,{tooltip},3,3,0,0,0,0}}",
        id = item.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = item.kind,
        name = quoted(item.name),
        after_name = item.after_name,
        data_path = item.data_path,
        payload = item.payload,
        context_menu = format_field_context_menu(item.context_menu_id, item.context_menu_name),
        tooltip = format_extended_tooltip(item.extended_tooltip_id, item.extended_tooltip_name),
    )
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
    /// Slot 6, `{1,0}` when the item names no format.
    pub(crate) format: &'a str,
    /// Slot 7: whether the field is one the user edits.
    pub(crate) editable: bool,
    /// Slot 8, the text colour.
    pub(crate) text_color: &'a str,
    /// Slot 9, the background colour.
    pub(crate) back_color: &'a str,
    /// Slot 10, the font.
    pub(crate) font: &'a str,
    /// Slot 12, the item's own event bindings.
    pub(crate) events: &'a str,
    /// Slot 15: `0` exactly when the item says `AutoMaxWidth` is false.
    pub(crate) auto_max_width: bool,
    /// Slot 16, `0` when the item names no maximum width.
    pub(crate) max_width: &'a str,
}

impl NativeLabelPayload<'_> {
    /// What a label that names no property of its own carries.
    pub(crate) const fn plain(editable: bool) -> Self {
        Self {
            width: "0",
            height: "0",
            horizontal_stretch: None,
            format: "{1,0}",
            editable,
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            events: "{0,1,0}",
            auto_max_width: true,
            max_width: "0",
        }
    }
}

pub(crate) fn format_label_payload(payload: &NativeLabelPayload<'_>) -> String {
    let stretch = match payload.horizontal_stretch {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{11,{width},{height},{stretch},2,2,{format},{editable},{text_color},{back_color},\
         {font},2,{events},{{3,4,{{0}}}},{{3,0,{{0}},0,1,0,{appearance}}},{auto_max_width},\
         {max_width},0,1,0}}",
        width = payload.width,
        height = payload.height,
        format = payload.format,
        editable = u8::from(payload.editable),
        text_color = payload.text_color,
        back_color = payload.back_color,
        font = payload.font,
        events = payload.events,
        appearance = DEFAULT_APPEARANCE_UUID,
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
    )
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

pub(crate) fn format_input_payload(payload: &NativeInputPayload<'_>) -> String {
    let stretch = |value: Option<bool>| match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{36,{{3,0}},{width},{height},{horizontal},{vertical},1,2,2,2,2,2,2,2,2,2,{{\"U\"}},{{\"U\"}},{mask},0,{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},0,0,2,3,00000000-0000-0000-0000-000000000000,{{5006,0}},{{0,0}},2,{format},{edit_format},2,1,0,{{\"Pattern\"}},1,{events},{text_color},{back_color},{border_color},{font},1,{{3,0,0}},0,{{1,0}},2,0,2,0,{auto_max_width},{max_width},0,1,0,0,0,0,0,0,0,0,0,{{0}},0,{{5007,0}},0}}",
        width = payload.width,
        height = payload.height,
        horizontal = stretch(payload.horizontal_stretch),
        vertical = stretch(payload.vertical_stretch),
        mask = quoted(payload.mask),
        format = payload.format,
        edit_format = payload.edit_format,
        events = payload.events,
        text_color = payload.text_color,
        back_color = payload.back_color,
        border_color = payload.border_color,
        font = payload.font,
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
    )
}

/// How a `<CheckBoxField>` draws itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeCheckBoxType {
    /// Also what a field that names no type carries.
    Auto,
    CheckBox,
    Tumbler,
    /// A switcher differs from `Auto` in one of the two slots that carry the
    /// type, which is why they cannot share a value.
    Switcher,
}

/// The `{11,…}` payload of a check box field -- the same wrapper a label
/// carries, with thirteen members instead of twenty.
///
/// Two of them carry the check box type. A candidate that fills those two and
/// copies the rest reproduces **all 10 385** check-box payloads of every ERP
/// УХ form body exactly, first time.
pub(crate) struct NativeCheckBoxPayload<'a> {
    pub(crate) check_box_type: NativeCheckBoxType,
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
}

impl NativeCheckBoxPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            check_box_type: NativeCheckBoxType::Auto,
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            format: "{1,0}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_check_box_payload(payload: &NativeCheckBoxPayload<'_>) -> String {
    let (kind, tail) = match payload.check_box_type {
        NativeCheckBoxType::Auto => ("0", "0"),
        NativeCheckBoxType::CheckBox => ("1", "1"),
        NativeCheckBoxType::Tumbler => ("2", "2"),
        NativeCheckBoxType::Switcher => ("0", "3"),
    };
    format!(
        "{{11,0,{text_color},{back_color},{kind},{format},{border_color},{font},0,0,0,2,{tail}}}",
        text_color = payload.text_color,
        back_color = payload.back_color,
        format = payload.format,
        border_color = payload.border_color,
        font = payload.font,
    )
}

/// How a `<RadioButtonField>` draws itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeRadioButtonType {
    /// Also what a field that names no type carries.
    Auto,
    RadioButtons,
    Tumbler,
}

/// The `{8,…}` payload of a radio-button field.
///
/// Two of its twelve members carry an XML property: the number of columns and
/// the type. A candidate that fills those two and copies the rest reproduces
/// **all 2 282** radio-button payloads of every ERP УХ form body exactly,
/// first time.
pub(crate) struct NativeRadioButtonPayload<'a> {
    /// Slot 2, `0` when the field names no column count.
    pub(crate) columns: &'a str,
    pub(crate) radio_button_type: NativeRadioButtonType,
    /// Slot 1, the choice list the field offers.
    pub(crate) choice_list: &'a str,
    /// Slot 3, the text colour; slot 5, the background; slot 8, the border.
    pub(crate) text_color: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
    /// Slot 4, the font.
    pub(crate) font: &'a str,
}

impl NativeRadioButtonPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            columns: "1",
            radio_button_type: NativeRadioButtonType::Auto,
            choice_list: "{3,0}",
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_radio_button_payload(payload: &NativeRadioButtonPayload<'_>) -> String {
    let kind = match payload.radio_button_type {
        NativeRadioButtonType::Auto => "0",
        NativeRadioButtonType::RadioButtons => "1",
        NativeRadioButtonType::Tumbler => "2",
    };
    format!(
        "{{8,{choice_list},{columns},{text_color},{font},{back_color},0,{kind},{border_color},0,0,2}}",
        choice_list = payload.choice_list,
        columns = payload.columns,
        text_color = payload.text_color,
        font = payload.font,
        back_color = payload.back_color,
        border_color = payload.border_color,
    )
}

/// The `{10,…}` payload of a picture field.
///
/// Five of its twenty-four members carry an XML property. All 2 202 records of
/// the corpus are reproduced exactly; the second round added the two
/// `<PictureSize>` spellings the first did not know, `AutoSizeIgnoreScale` and
/// `ByFontSize`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn format_picture_payload(
    width: &str,
    height: &str,
    picture_size: Option<&str>,
    max_width: &str,
    max_height: &str,
    picture: &str,
    title: &str,
    text_color: &str,
    back_color: &str,
    font: &str,
    border: &str,
    events: &str,
    third: &str,
    fourth: &str,
    seventeenth: &str,
    twentieth: &str,
    twenty_second: &str,
) -> String {
    let size = match picture_size {
        Some("Stretch") => "1",
        Some("Proportionally") => "2",
        Some("AutoSize") => "4",
        Some("AutoSizeIgnoreScale") => "6",
        Some("ByFontSize") => "7",
        _ => "0",
    };
    format!(
        "{{10,{width},{height},{third},{fourth},{picture},{size},0,0,{title},{text_color},{back_color},{font},{border},0,{events},{seventeenth},{max_width},0,{twentieth},{max_height},{twenty_second},0,100}}"
    )
}

/// The `{13,…}` payload of a spreadsheet document field.
///
/// Six of its thirty-two members carry an XML property -- the two sizes, the
/// two maxima and the two scroll bars, where a field naming neither scroll bar
/// carries 2. All 942 records of the corpus are reproduced exactly, first
/// time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn format_spreadsheet_payload(
    width: &str,
    height: &str,
    max_width: &str,
    max_height: &str,
    vertical_scroll_bar: Option<bool>,
    horizontal_scroll_bar: Option<bool>,
    middle: &str,
    events: &str,
    tail: &str,
) -> String {
    let bar = |value: Option<bool>| match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{13,{width},{height},{middle},{max_width},0,{max_height},0,0,{events},{tail},{vertical},{horizontal},1,2}}",
        vertical = bar(vertical_scroll_bar),
        horizontal = bar(horizontal_scroll_bar),
    )
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
        context_menu = format_field_context_menu(
            decoration.context_menu_id,
            decoration.context_menu_name
        ),
        tooltip = format_extended_tooltip(
            decoration.extended_tooltip_id,
            decoration.extended_tooltip_name
        ),
    )
}

/// The namespace a form *command* id lives in, which is not the item one.
pub(crate) const FORM_COMMAND_NAMESPACE_UUID: &str = "409b9a53-7f7e-4178-86c1-33176c7c7a7a";

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
/// the two `<UseAlways>` blocks and the two that carry the `<View>` and
/// `<Edit>` restrictions. Everything else is read from the source:
///
/// - member 10 is `<MainAttribute>`;
/// - member 11 is `<SavedData>`;
/// - member 12 is `<FillCheck>`, 1 for `ShowError`.
pub(crate) struct NativeFormAttribute<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    /// Already formatted -- `{1,0}` for an attribute with no title.
    pub(crate) title: &'a str,
    /// Already formatted -- `{"Pattern"}` for an attribute the form does not
    /// type, and the pattern with its reference list for one it does.
    pub(crate) type_pattern: &'a str,
    /// The two `<UseAlways>` blocks, `{0,{0,{"B",1},0}}` when the attribute
    /// restricts nothing.
    pub(crate) use_always: [&'a str; 2],
    /// The `<View>` and `<Edit>` restrictions, `{0,0}` when there are none.
    pub(crate) restrictions: [&'a str; 2],
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
            use_always: ["{0,{0,{\"B\",1},0}}", "{0,{0,{\"B\",1},0}}"],
            restrictions: ["{0,0}", "{0,0}"],
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
        "{{9,{{{id}}},0,{name},{title},{type_pattern},{use_always_one},{use_always_two},\
         {view},{edit},{main},{saved},{fill_check},{count}{columns},{first},{second}}}",
        id = attribute.id,
        name = quoted(attribute.name),
        title = attribute.title,
        type_pattern = attribute.type_pattern,
        use_always_one = attribute.use_always[0],
        use_always_two = attribute.use_always[1],
        view = attribute.restrictions[0],
        edit = attribute.restrictions[1],
        main = u8::from(attribute.main_attribute),
        saved = u8::from(attribute.saved_data),
        fill_check = u8::from(attribute.fill_check),
        count = attribute.columns.len(),
        first = attribute.trailing[0],
        second = attribute.trailing[1],
    )
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

/// How a `<UsualGroup>` arranges what it holds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGroupArrangement {
    /// `<Group>Vertical</Group>`.
    Vertical,
    /// `<Group>Horizontal</Group>`.
    Horizontal,
    /// `<Group>AlwaysHorizontal</Group>`, which differs from `Horizontal` in
    /// one of the two slots that carry it.
    AlwaysHorizontal,
    /// `<Group>HorizontalIfPossible</Group>`, which is also what a group that
    /// names no arrangement at all carries.
    HorizontalIfPossible,
}

/// How a `<UsualGroup>` behaves.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGroupBehavior {
    Usual,
    Collapsible,
    PopUp,
    /// What a group that names no behaviour carries, which is not `Usual`.
    Unnamed,
}

/// How a `<UsualGroup>` separates itself from what is around it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGroupSeparation {
    None,
    Strong,
    /// Also what a group that names no representation carries.
    Weak,
    Normal,
}

/// The `{29,…}` payload of a usual group, member by member.
///
/// Seven of its twenty-nine members carry an XML property. Measured the same
/// way as the field payloads: a candidate that fills these seven and copies
/// the rest reproduces **every one** of the usual-group payloads it was run
/// against exactly.
///
/// The refinement that got it there was about defaults, not about which slot
/// is which: a group that names no `<Group>` carries what
/// `HorizontalIfPossible` carries, a group that names no `<Representation>`
/// carries what `WeakSeparation` carries, and a group that names no
/// `<Behavior>` carries something that is *not* what `Usual` carries.
pub(crate) struct NativeUsualGroupPayload<'a> {
    pub(crate) separation: NativeGroupSeparation,
    pub(crate) behavior: NativeGroupBehavior,
    /// Slot 19: `Use` -> 0, `DontUse` -> 1, absent -> 2.
    pub(crate) through_align: Option<bool>,
    pub(crate) arrangement: NativeGroupArrangement,
    /// Slot 5, the group's own picture binding, `{0}` by default.
    pub(crate) picture: &'a str,
    /// Slot 9, the background colour, `{3,4,{0}}` by default.
    pub(crate) back_color: &'a str,
    /// Slot 14, the group's title, `{1,0}` by default.
    pub(crate) title: &'a str,
    /// Slot 4, which varies 0/1 over the corpus and whose property is not read
    /// yet, so the caller supplies it.
    pub(crate) fourth: &'a str,
}

impl NativeUsualGroupPayload<'_> {
    /// What a group that names none of these carries.
    pub(crate) const fn plain() -> Self {
        Self {
            separation: NativeGroupSeparation::Weak,
            behavior: NativeGroupBehavior::Unnamed,
            through_align: None,
            arrangement: NativeGroupArrangement::HorizontalIfPossible,
            picture: "{0}",
            back_color: "{3,4,{0}}",
            title: "{1,0}",
            fourth: "0",
        }
    }
}

pub(crate) fn format_usual_group_payload(payload: &NativeUsualGroupPayload<'_>) -> String {
    let separation = match payload.separation {
        NativeGroupSeparation::None => "0",
        NativeGroupSeparation::Strong => "1",
        NativeGroupSeparation::Weak => "2",
        NativeGroupSeparation::Normal => "3",
    };
    let (collapsible, behavior, behavior_tail) = match payload.behavior {
        NativeGroupBehavior::Usual => ("0", "0", "0"),
        NativeGroupBehavior::Collapsible => ("1", "1", "1"),
        NativeGroupBehavior::PopUp => ("1", "2", "2"),
        NativeGroupBehavior::Unnamed => ("1", "0", "3"),
    };
    let through_align = match payload.through_align {
        Some(true) => "0",
        Some(false) => "1",
        None => "2",
    };
    let (arrangement, arrangement_tail) = match payload.arrangement {
        NativeGroupArrangement::Vertical => ("0", "0"),
        NativeGroupArrangement::Horizontal => ("1", "1"),
        NativeGroupArrangement::AlwaysHorizontal => ("1", "3"),
        NativeGroupArrangement::HorizontalIfPossible => ("2", "2"),
    };
    format!(
        "{{29,{arrangement},0,{separation},{fourth},{picture},{{1,0}},{{\"Pattern\"}},\"\",{back_color},{collapsible},0,0,1,{title},0,0,3,3,{through_align},0,1,{arrangement},{{3,4,{{0}}}},{behavior},2,0,{arrangement_tail},{behavior_tail}}}",
        fourth = payload.fourth,
        picture = payload.picture,
        back_color = payload.back_color,
        title = payload.title,
    )
}

/// The `{2,…}` payload of a button group.
///
/// Four members, one of which carries `<Representation>`. All 21 651 of the
/// corpus are reproduced exactly.
pub(crate) fn format_button_group_payload(command_source: &str, representation: Option<&str>) -> String {
    let representation = match representation {
        Some("Usual") => "1",
        Some("Compact") => "2",
        _ => "0",
    };
    format!("{{2,{command_source},2,{representation}}}")
}

/// The `{1,…}` payload of a command bar.
///
/// Three members, one of which carries `<HorizontalLocation>`. All 3 233 of
/// the corpus are reproduced exactly.
pub(crate) fn format_command_bar_payload(horizontal_location: Option<&str>, command_source: &str) -> String {
    let location = match horizontal_location {
        Some("Auto") => "3",
        Some("Right") => "2",
        Some("Center") => "1",
        _ => "0",
    };
    format!("{{1,{location},{command_source}}}")
}

/// The `{4,…}` payload of a `<Pages>` group.
///
/// Six members, two of which carry `<PagesRepresentation>` -- the same value
/// in both, except that a group naming none carries 1 in the first and 6 in
/// the second. All 4 329 of the corpus are reproduced exactly.
pub(crate) fn format_pages_payload(representation: Option<&str>, events: &str, fourth: &str) -> String {
    let code = |value: Option<&str>| match value {
        Some("None") => "0",
        Some("TabsOnTop") => "1",
        Some("TabsOnBottom") => "2",
        Some("TabsOnLeftHorizontal") => "3",
        Some("Swipe") => "5",
        _ => "",
    };
    let first = match code(representation) {
        "" => "1",
        value => value,
    };
    let second = match code(representation) {
        "" => "6",
        value => value,
    };
    format!("{{4,{first},{events},2,{fourth},{second}}}")
}

/// The `{7,…}` payload of a `<Popup>`.
///
/// Nine members, one of which carries `<Representation>`; a popup that names
/// none carries 3. All 10 642 of the corpus are reproduced exactly.
pub(crate) fn format_popup_payload(
    picture: &str,
    command_source: &str,
    representation: Option<&str>,
    back_color: &str,
    border_color: &str,
) -> String {
    let representation = match representation {
        Some("Text") => "0",
        Some("Picture") => "1",
        Some("PictureAndText") => "2",
        _ => "3",
    };
    format!("{{7,{picture},{command_source},2,{representation},0,0,{back_color},{border_color}}}")
}

/// The `{2,…}` payload of a `<ColumnGroup>` -- the same wrapper a button group
/// carries, with twelve members instead of four.
///
/// One of them carries `<Group>`: `Horizontal` is 0, `InCell` is 2, and a
/// group naming none carries 1. All 7 076 of the corpus are reproduced
/// exactly.
pub(crate) fn format_column_group_payload(
    group: Option<&str>,
    second: &str,
    third: &str,
    fourth: &str,
    picture: &str,
    back_color: &str,
    title: &str,
) -> String {
    let group = match group {
        Some("Horizontal") => "0",
        Some("InCell") => "2",
        _ => "1",
    };
    format!(
        "{{2,{group},{second},{third},{fourth},{picture},{back_color},{{0}},{{\"Pattern\"}},\"\",{title},0}}"
    )
}

/// What a `{18,…}` page payload carries.
///
/// Three of its twenty members carry `<Group>`: one says whether the page
/// names an arrangement at all, and two carry which one, differing on
/// `AlwaysHorizontal`. All 11 804 of the corpus are reproduced exactly.
pub(crate) struct NativePagePayload<'a> {
    pub(crate) group: Option<&'a str>,
    pub(crate) picture: &'a str,
    pub(crate) third: &'a str,
    pub(crate) data_path: &'a str,
    pub(crate) title: &'a str,
    pub(crate) sixth: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) tenth: &'a str,
    pub(crate) eleventh: &'a str,
    pub(crate) twelfth: &'a str,
    pub(crate) thirteenth: &'a str,
    pub(crate) fourteenth: &'a str,
    pub(crate) fifteenth: &'a str,
    pub(crate) border_color: &'a str,
    pub(crate) font: &'a str,
}

impl NativePagePayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            group: None,
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            third: "0",
            data_path: "{0}",
            title: "{1,0}",
            sixth: "1",
            back_color: "{3,4,{0}}",
            tenth: "0",
            eleventh: "0",
            twelfth: "3",
            thirteenth: "3",
            fourteenth: "0",
            fifteenth: "0",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_page_payload(payload: &NativePagePayload<'_>) -> String {
    let named = u8::from(payload.group.is_some());
    let (horizontal, horizontal_tail) = match payload.group {
        Some("Horizontal") => ("1", "1"),
        Some("AlwaysHorizontal") => ("1", "3"),
        Some("HorizontalIfPossible") => ("2", "2"),
        Some("Vertical") | None => ("0", "0"),
        Some(_) => ("0", "0"),
    };
    format!(
        "{{18,{picture},{named},{third},{data_path},{title},{sixth},{{\"Pattern\"}},\"\",{back_color},{tenth},{eleventh},{twelfth},{thirteenth},{fourteenth},{fifteenth},{horizontal},{horizontal_tail},{border_color},{font}}}",
        picture = payload.picture,
        third = payload.third,
        data_path = payload.data_path,
        title = payload.title,
        sixth = payload.sixth,
        back_color = payload.back_color,
        tenth = payload.tenth,
        eleventh = payload.eleventh,
        twelfth = payload.twelfth,
        thirteenth = payload.thirteenth,
        fourteenth = payload.fourteenth,
        fifteenth = payload.fifteenth,
        border_color = payload.border_color,
        font = payload.font,
    )
}

/// What a `{22,…}` group record needs beyond the frame every group shares.
///
/// `kind` is the marker that says which group this is -- 0 command-bar group,
/// 1 submenu, 2 ordinary group, 3 pages, 4 page, 5 usual group, 6 button
/// group, 7 navigator -- and it decides the shape of `payload`. Censused over
/// all 12 515 ERP УХ form bodies: each kind has exactly one payload structure
/// with a fixed member count (29 for kind 5, 20 for kind 4, 4 for kind 6, and
/// so on), varying only in a handful of scalar slots. The payload arrives
/// already formatted for the same reason a field's data path does: the slot
/// that carries an XML property is named where that property is read, not
/// here.
pub(crate) struct NativeGroupItem<'a> {
    pub(crate) id: &'a str,
    pub(crate) kind: u8,
    pub(crate) name: &'a str,
    /// Already formatted -- see [`format_russian_title`].
    pub(crate) title: &'a str,
    pub(crate) tooltip_title: &'a str,
    /// The frame's four varying members, in the order they appear.
    pub(crate) frame: NativeGroupFrame,
    pub(crate) payload: &'a str,
    /// `(group uuid, child record)` in the order the body stores them.
    pub(crate) children: &'a [(&'a str, String)],
    /// Already formatted -- see [`format_extended_tooltip`].
    pub(crate) extended_tooltip: &'a str,
}

/// The four members of the group frame that are not constant.
#[derive(Clone, Copy, Default)]
pub(crate) struct NativeGroupFrame {
    pub(crate) first: u32,
    pub(crate) second: u32,
    pub(crate) third: u32,
    pub(crate) fourth: u32,
}

impl NativeGroupFrame {
    /// The values 68 645 of the 68 836 kind-5 records carry.
    pub(crate) const fn usual() -> Self {
        Self {
            first: 0,
            second: 0,
            third: 2,
            fourth: 2,
        }
    }
}

/// The `{22,…}` record of a group, with its children in place.
pub(crate) fn format_group_item(group: &NativeGroupItem<'_>) -> String {
    let mut children = String::new();
    for (group_uuid, record) in group.children {
        children.push(',');
        children.push_str(group_uuid);
        children.push(',');
        children.push_str(record);
    }
    format!(
        "{{22,{{{id},{ns}}},0,0,0,{kind},{name},{title},{tooltip_title},{first},1,0,{second},0,\
         {third},{fourth},{{3,4,{{0}}}},{{7,3,0,1,100}},{{0,0,0}},1,{payload},{count}{children},\
         1,0,1,{tooltip},0,3,3,0}}",
        id = group.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = group.kind,
        name = quoted(group.name),
        title = group.title,
        tooltip_title = group.tooltip_title,
        first = group.frame.first,
        second = group.frame.second,
        third = group.frame.third,
        fourth = group.frame.fourth,
        payload = group.payload,
        count = group.children.len(),
        tooltip = group.extended_tooltip,
    )
}

/// What the `{50,…}` root layout of a form body carries.
///
/// The childless root is 48 members. Over the roots of the first 600 ERP УХ
/// form bodies that carry no child item, forty of those members are the same
/// in every one; the eight below are not. A root that carries children appends
/// them after the command bar, which this writer does not do yet because the
/// count encoding there is not measured.
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
    /// `<SettingsStorage>`. Naming one refuses the head.
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
    if head.settings_storage.is_some() {
        return None;
    }
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
        "00000000-0000-0000-0000-000000000000".to_string(),
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
/// The one refusal is that mobile command bar: its content reaches the tail as
/// `{50,1,"",{"N",<n>}}`, a reference into the body's own value table, which
/// the source alone cannot resolve. All 78 forms that differed carried it, and
/// no other form did.
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
        }
    }
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
        "{50,0}".to_string(),
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
            visible_tuple: true,
            after_name: "1,0",
            data_path: "{2,{1},{3}}",
            payload: &format_plain_field_payload(false),
            context_menu_id: "21",
            context_menu_name: "ПериодЗакупокКонтекстноеМеню",
            extended_tooltip_id: "22",
            extended_tooltip_name: "ПериодЗакупокРасширеннаяПодсказка",
        });
        assert_eq!(
            period,
            "{37,{20,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},1,\"ПериодЗакупок\",1,0,{1,0},{1,0},{2,{1},{3}},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},\"\",-1,-1,1,0,\"\"},{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{7,3,0,1,100},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{11,0,0,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0},{0,1,0},1,{22,{21,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ПериодЗакупокКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,{\"Pattern\"},{\"Pattern\"},\"\",\"\",{0},0,0,1,{12,{22,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ПериодЗакупокРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},3,3,0,0,0,0}"
        );

        let supplier = format_field_item(&NativeFieldItem {
            id: "46",
            kind: 1,
            name: "АнкетаПоставщика",
            visible_tuple: true,
            after_name: "1,0",
            data_path: "{1,{2}}",
            payload: &format_plain_field_payload(true),
            context_menu_id: "47",
            context_menu_name: "АнкетаПоставщикаКонтекстноеМеню",
            extended_tooltip_id: "48",
            extended_tooltip_name: "АнкетаПоставщикаРасширеннаяПодсказка",
        });
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
            visible_tuple: false,
            after_name: "0,0",
            data_path: "{1,{3}}",
            payload: "{13,100,10,1,1,0,0,1,1,0,0,1,0,0,1,{3,4,{0}},1,1,{0,1,0},0,1,0,0,1,0,0,0,0,1,1,1,2}",
            context_menu_id: "10",
            context_menu_name: "РезультатКонтекстноеМеню",
            extended_tooltip_id: "12",
            extended_tooltip_name: "РезультатРасширеннаяПодсказка",
        });
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
            format_field_context_menu("18", "ЛотКонтекстноеМеню"),
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
                frame: NativeGroupFrame::usual(),
                payload,
                children: &[],
                extended_tooltip: &tooltip,
            });
            if !expected_head.is_empty() {
                assert!(
                    record.starts_with(expected_head),
                    "kind {kind} head differs:\n{record}"
                );
            }
            assert!(record.ends_with(&format!("{tooltip},0,3,3,0}}")));
        }
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
            frame: NativeGroupFrame::usual(),
            payload: "{1,0,{0}}",
            children: &[("a9f3b1ac-f51b-431e-b102-55a69acdecad", child.clone())],
            extended_tooltip: &tooltip,
        });

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
        let context_menu = format_field_context_menu("57", "ОтборКонтекстноеМеню");
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
            &format_field_context_menu("61", "ОтборСтрокаПоискаКонтекстноеМеню"),
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
            &format_field_context_menu("67", "ОтборУправлениеПоискомКонтекстноеМеню"),
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
            visible_tuple: false,
            after_name: "1,0",
            data_path: "{1,{2}}",
            payload: &format_plain_field_payload(true),
            context_menu_id: "2",
            context_menu_name: "AM",
            extended_tooltip_id: "3",
            extended_tooltip_name: "AT",
        });
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
            format_input_payload(&NativeInputPayload::plain()),
            "{36,{3,0},0,0,2,2,1,2,2,2,2,2,2,2,2,2,{\"U\"},{\"U\"},\"\",0,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,2,3,00000000-0000-0000-0000-000000000000,{5006,0},{0,0},2,{1,0},{1,0},2,1,0,{\"Pattern\"},1,{0,1,0},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},1,{3,0,0},0,{1,0},2,0,2,0,1,0,0,1,0,0,0,0,0,0,0,0,0,{0},0,{5007,0},0}"
        );
        assert!(format_input_payload(&NativeInputPayload {
            width: "25",
            ..NativeInputPayload::plain()
        })
        .starts_with("{36,{3,0},25,0,2,2,"));
        assert!(format_input_payload(&NativeInputPayload {
            auto_max_width: false,
            max_width: "28",
            ..NativeInputPayload::plain()
        })
        .contains(",2,0,2,0,0,28,0,1,0,"));
        assert!(format_input_payload(&NativeInputPayload {
            mask: "999-999-999 99",
            ..NativeInputPayload::plain()
        })
        .contains(",{\"U\"},\"999-999-999 99\",0,"));
    }

    /// The payload of `ГруппаШапка`, a usual group of an ERP УХ form body,
    /// exactly as that body stores it, and the defaults a group that names
    /// nothing carries.
    #[test]
    fn writes_the_usual_group_payloads_the_platform_stores() {
        assert_eq!(
            format_usual_group_payload(&NativeUsualGroupPayload {
                behavior: NativeGroupBehavior::Usual,
                arrangement: NativeGroupArrangement::HorizontalIfPossible,
                separation: NativeGroupSeparation::Weak,
                fourth: "1",
                ..NativeUsualGroupPayload::plain()
            }),
            "{29,2,0,2,1,{0},{1,0},{\"Pattern\"},\"\",{3,4,{0}},0,0,0,1,{1,0},0,0,3,3,2,0,1,2,{3,4,{0}},0,2,0,2,0}"
        );
        // A group that names no arrangement carries what `HorizontalIfPossible`
        // carries, and one that names no behaviour carries what `Usual` does
        // not.
        let plain = format_usual_group_payload(&NativeUsualGroupPayload::plain());
        assert!(plain.starts_with("{29,2,0,2,0,"));
        assert!(plain.ends_with(",1,2,{3,4,{0}},0,2,0,2,3}"));
        let vertical = format_usual_group_payload(&NativeUsualGroupPayload {
            arrangement: NativeGroupArrangement::Vertical,
            ..NativeUsualGroupPayload::plain()
        });
        assert!(vertical.starts_with("{29,0,0,2,0,"));
        assert!(vertical.ends_with(",0,3}"));
    }

    /// The check-box payload 9 906 records carry unchanged, the radio-button
    /// payload, and the slots that carry each field's type.
    #[test]
    fn writes_the_check_box_and_radio_payloads_the_platform_stores() {
        assert_eq!(
            format_check_box_payload(&NativeCheckBoxPayload::plain()),
            "{11,0,{3,4,{0}},{3,4,{0}},0,{1,0},{3,4,{0}},{7,3,0,1,100},0,0,0,2,0}"
        );
        // `Auto` and `Switcher` share the first slot and differ in the last.
        assert!(format_check_box_payload(&NativeCheckBoxPayload {
            check_box_type: NativeCheckBoxType::Switcher,
            ..NativeCheckBoxPayload::plain()
        })
        .ends_with(",0,0,0,2,3}"));
        assert!(format_check_box_payload(&NativeCheckBoxPayload {
            check_box_type: NativeCheckBoxType::Tumbler,
            ..NativeCheckBoxPayload::plain()
        })
        .contains("{3,4,{0}},2,{1,0},"));

        assert_eq!(
            format_radio_button_payload(&NativeRadioButtonPayload::plain()),
            "{8,{3,0},1,{3,4,{0}},{7,3,0,1,100},{3,4,{0}},0,0,{3,4,{0}},0,0,2}"
        );
        assert!(format_radio_button_payload(&NativeRadioButtonPayload {
            columns: "3",
            radio_button_type: NativeRadioButtonType::Tumbler,
            ..NativeRadioButtonPayload::plain()
        })
        .starts_with("{8,{3,0},3,{3,4,{0}},{7,3,0,1,100},{3,4,{0}},0,2,"));
    }

    /// The default payload of each of the six group kinds the corpus closed
    /// in one pass, exactly as those bodies store them.
    #[test]
    fn writes_the_remaining_group_payloads_the_platform_stores() {
        assert_eq!(format_button_group_payload("{0}", None), "{2,{0},2,0}");
        assert_eq!(format_button_group_payload("{0}", Some("Compact")), "{2,{0},2,2}");
        assert_eq!(format_command_bar_payload(None, "{0}"), "{1,0,{0}}");
        assert_eq!(format_command_bar_payload(Some("Auto"), "{0}"), "{1,3,{0}}");
        assert_eq!(format_pages_payload(None, "{0,1,0}", "0"), "{4,1,{0,1,0},2,0,6}");
        assert_eq!(
            format_pages_payload(Some("None"), "{0,1,0}", "0"),
            "{4,0,{0,1,0},2,0,0}"
        );
        assert_eq!(
            format_popup_payload(
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "{0}",
                None,
                "{3,4,{0}}",
                "{3,4,{0}}"
            ),
            "{7,{4,0,{0},\"\",-1,-1,1,0,\"\"},{0},2,3,0,0,{3,4,{0}},{3,4,{0}}}"
        );
        assert_eq!(
            format_column_group_payload(
                None,
                "1",
                "0",
                "3",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "{3,4,{0}}",
                "{1,0}"
            ),
            "{2,1,1,0,3,{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{0},{\"Pattern\"},\"\",{1,0},0}"
        );
        assert_eq!(
            format_page_payload(&NativePagePayload::plain()),
            "{18,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,{0},{1,0},1,{\"Pattern\"},\"\",{3,4,{0}},0,0,3,3,0,0,0,0,{3,4,{0}},{7,3,0,1,100}}"
        );
        // `AlwaysHorizontal` and `Horizontal` differ in the second of the two
        // slots that carry the arrangement.
        assert!(format_page_payload(&NativePagePayload {
            group: Some("AlwaysHorizontal"),
            ..NativePagePayload::plain()
        })
        .contains(",0,0,1,3,{3,4,{0}},"));
        assert!(format_page_payload(&NativePagePayload {
            group: Some("Horizontal"),
            ..NativePagePayload::plain()
        })
        .contains(",0,0,1,1,{3,4,{0}},"));
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
            format_picture_payload(
                "2", "0", size, "0", "0",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}", "{1,0}", "{3,4,{0}}", "{3,4,{0}}",
                "{7,3,0,1,100}", "{3,0,{0},1,1,0,48312c09-257f-4b29-b280-284dd89efc1e}",
                "{0,1,0}", "1", "1", "1", "1", "0",
            )
        };
        assert!(picture(None).contains("-1,-1,1,0,\"\"},0,0,0,{1,0},"));
        assert!(picture(Some("ByFontSize")).contains("-1,-1,1,0,\"\"},7,0,0,{1,0},"));
        assert!(picture(Some("AutoSizeIgnoreScale")).contains("-1,-1,1,0,\"\"},6,0,0,{1,0},"));

        // A spreadsheet field that names neither scroll bar carries 2 in both
        // of the slots that hold them.
        let sheet = |vertical, horizontal| {
            format_spreadsheet_payload(
                "50", "10", "0", "0", vertical, horizontal,
                "1,1,0", "0,0", "1,1",
            )
        };
        assert!(sheet(None, None).ends_with(",2,2,1,2}"));
        assert!(sheet(Some(true), Some(false)).ends_with(",1,0,1,2}"));
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

        // A form that names a settings storage is refused: the head would have
        // to carry that object's uuid, which only the configuration knows.
        assert_eq!(
            format_root_head(&NativeRootHead {
                settings_storage: Some("SettingsStorage.Общие"),
                ..NativeRootHead::default()
            }),
            None
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
        assert!(before_write("DocumentObject")
            .unwrap()
            .contains("8a5894c9-d2ff-4c1d-b433-89cc352bbfbc"));
        assert!(before_write("CatalogObject")
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

    /// The two font shapes the corpus pins down, and the refusal of the one
    /// it does not.
    #[test]
    fn writes_the_fonts_the_platform_stores() {
        let none = |_: &str| None;
        assert_eq!(
            format_native_font(None, None, false, none).as_deref(),
            Some("{7,3,0,1,100}")
        );
        assert_eq!(
            format_native_font(Some("StyleItem"), Some("style:ШрифтНовостей"), false, |name| {
                (name == "ШрифтНовостей")
                    .then(|| "db43d980-40e2-4f50-b17b-fac3bd9e2773".to_string())
            })
            .as_deref(),
            Some("{7,2,0,{0,db43d980-40e2-4f50-b17b-fac3bd9e2773},1,100}")
        );
        // A font that also sets bold or a size, a Windows font, and a platform
        // style font are all refused rather than guessed.
        assert_eq!(
            format_native_font(Some("StyleItem"), Some("style:ШрифтНовостей"), true, |_| Some(
                "db43d980-40e2-4f50-b17b-fac3bd9e2773".to_string()
            )),
            None
        );
        assert_eq!(
            format_native_font(Some("WindowsFont"), Some("sys:DefaultGUIFont"), false, none),
            None
        );
        assert_eq!(
            format_native_font(Some("StyleItem"), Some("style:FormBackColor"), false, |_| Some(
                "x".to_string()
            )),
            None
        );
    }

    /// A name that carries a quote is escaped the way every other 1C string in
    /// a body is, rather than being written raw.
    #[test]
    fn escapes_a_quoted_name() {
        assert!(format_extended_tooltip("1", "a\"b").contains("\"a\"\"b\""));
    }
}
