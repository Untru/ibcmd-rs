//! Style codes: the standard style items and the web and style palettes.

pub fn standard_style_item_for_code(code: i32) -> Option<(usize, &'static str)> {
    let name = match code {
        -1 => "FormBackColor",
        -11 => "FormTextColor",
        -3 => "ButtonBackColor",
        -15 => "ButtonTextColor",
        -7 => "FieldBackColor",
        -13 => "FieldTextColor",
        -21 => "FieldSelectionBackColor",
        -10 => "FieldSelectedTextColor",
        -14 => "FieldAlternativeBackColor",
        -23 => "ToolTipBackColor",
        -24 => "ToolTipTextColor",
        -16 => "SpecialTextColor",
        -17 => "NegativeTextColor",
        -22 => "BorderColor",
        -25 => "ReportHeaderBackColor",
        -26 => "ReportGroup1BackColor",
        -27 => "ReportGroup2BackColor",
        -28 => "ReportLineColor",
        -18 => "ControlBorder",
        -20 => "TextFont",
        -30 => "SmallTextFont",
        -31 => "NormalTextFont",
        -32 => "LargeTextFont",
        -33 => "ExtraLargeTextFont",
        -34 => "ButtonBorderColor",
        -35 => "TableHeaderBackColor",
        -36 => "TableHeaderTextColor",
        -37 => "TableFooterBackColor",
        -38 => "TableFooterTextColor",
        -42 => "NavigationColor",
        -43 => "AuxiliaryNavigationColor",
        -44 => "ActivityColor",
        -47 => "ImportantColor",
        // Platform 8.5 standard style fonts. 8.5.1.1150 BSP: every form item
        // and style item that carries exactly one style font pairs the code
        // with the native `ref` without exception (`fontcodes.py`).
        -50 => "TitleLevel3",
        -52 => "TitleLevel5",
        -53 => "TextLevel1",
        -54 => "TextLevel2",
        -55 => "TextLevel3",
        -56 => "SubtitleLevel1",
        -59 => "TitleLevel2",
        _ => return None,
    };
    let order = STANDARD_STYLE_ITEM_CODES
        .iter()
        .position(|item_code| *item_code == code)?;
    Some((order, name))
}

/// The inverse of the style reader's single standard-item table. Keeping the
/// source writer on this table covers the 8.5 colour/font additions too.
pub fn standard_style_item_code_for_name(name: &str) -> Option<i32> {
    STANDARD_STYLE_ITEM_CODES
        .iter()
        .copied()
        .find(|code| standard_style_item_for_code(*code).is_some_and(|(_, found)| found == name))
}

const STANDARD_STYLE_ITEM_CODES: &[i32] = &[
    -1, -11, -3, -15, -7, -13, -21, -10, -14, -23, -24, -16, -17, -22, -25, -26, -27, -28, -18,
    -20, -30, -31, -32, -33, -34, -35, -36, -37, -38, -44, -42, -43, -47, -50, -52, -53, -54, -55,
    -56, -59,
];

/// Codes 49/61/78/110/115/117/123/127/129/131 (GhostWhite/Lavender/
/// LightSteelBlue/PaleTurquoise/Pink/PowderBlue/Salmon/Sienna/SkyBlue/
/// SlateGray) are evidenced on all 10 `StyleItems` real ERP УХ 3.2.12.6
/// bytes leave `missing` at 2ccd98f for exactly this reason: `parse_style_
/// item_properties_from_text` -> `parse_style_color_value` -> here returns
/// `None` for any code this function does not recognize, which refuses the
/// *entire* StyleItem (not a partial result) -- one unmapped color code is
/// indistinguishable from "no legacy family decoder recognized this
/// storage entry" in the export report. Each value here was read directly
/// off the platform's own native `<Value xsi:type="v8ui:Color">web:
/// <Name></Value>` for that exact object (not inferred from .NET's
/// KnownColor numbering, which this table's existing gaps/duplicates --
/// e.g. `27`/`31` both `DarkGreen`, `94`/`105` both `Orange` -- already show
/// does not hold platform-wide).
pub fn style_web_color_name(code: i32) -> Option<&'static str> {
    match code {
        6 => Some("web:Beige"),
        8 => Some("web:Black"),
        10 => Some("web:Blue"),
        20 => Some("web:Cream"),
        21 => Some("web:Crimson"),
        26 => Some("web:DarkGray"),
        27 => Some("web:DarkGreen"),
        31 => Some("web:DarkGreen"),
        23 => Some("web:DarkBlue"),
        33 => Some("web:DarkRed"),
        37 => Some("web:DarkSlateGray"),
        // Four indexes the style items of the stand attest and this table did
        // not carry. Each is read off the platform's own `<Value>` for the
        // same object: `43` on `do` `StyleItems/ИзбранныйЭлемент` and two more,
        // `62` on `СерияДиаграммыВиджетаРозовый`, `77` on
        // `ПользовательБезУчетнойЗаписи`, `143` on `МК_ЦветТекстаАкцентнойКнопки`
        // and two more. `43` and `143` are the same values the form palette
        // already carried in front of this table.
        43 => Some("web:DodgerBlue"),
        44 => Some("web:FireBrick"),
        45 => Some("web:FloralWhite"),
        46 => Some("web:ForestGreen"),
        50 => Some("web:Gold"),
        51 => Some("web:Goldenrod"),
        48 => Some("web:Gainsboro"),
        49 => Some("web:GhostWhite"),
        52 => Some("web:Gray"),
        53 => Some("web:Green"),
        55 => Some("web:HoneyDew"),
        61 => Some("web:Lavender"),
        62 => Some("web:LavenderBlush"),
        // `uh` `StyleItems/ИзмененнаяНастройка` writes `web:LemonChiffon`
        // for index 64; the table read it as `LightCoral`, a name it already
        // gives 66.
        64 => Some("web:LemonChiffon"),
        65 => Some("web:LightBlue"),
        66 => Some("web:LightCoral"),
        67 => Some("web:LightCyan"),
        68 => Some("web:LightGoldenRod"),
        69 => Some("web:LightGoldenRodYellow"),
        71 => Some("web:LightGray"),
        72 => Some("web:LightPink"),
        // `uh` `StyleItems/ЦветКритичногоНедостаткаСредств` stores code 73 and
        // the platform writes `web:LightSalmon` for it; one unmapped code
        // refuses the whole StyleItem, which is why that object was missing.
        73 => Some("web:LightSalmon"),
        77 => Some("web:LightSlateGray"),
        78 => Some("web:LightSteelBlue"),
        79 => Some("web:LightYellow"),
        82 => Some("web:Linen"),
        84 => Some("web:Maroon"),
        86 => Some("web:MediumBlue"),
        87 => Some("web:MediumGray"),
        88 => Some("web:MediumGreen"),
        94 => Some("web:Orange"),
        105 => Some("web:Orange"),
        97 => Some("web:MintCream"),
        98 => Some("web:MistyRose"),
        96 => Some("web:Moccasin"),
        99 => Some("web:Moccasin"),
        110 => Some("web:PaleTurquoise"),
        115 => Some("web:Pink"),
        117 => Some("web:PowderBlue"),
        119 => Some("web:Red"),
        120 => Some("web:RosyBrown"),
        121 => Some("web:RoyalBlue"),
        123 => Some("web:Salmon"),
        127 => Some("web:Sienna"),
        128 => Some("web:Silver"),
        129 => Some("web:SkyBlue"),
        130 => Some("web:SlateBlue"),
        131 => Some("web:SlateGray"),
        134 => Some("web:SteelBlue"),
        138 => Some("web:Tomato"),
        140 => Some("web:Violet"),
        141 => Some("web:VioletRed"),
        143 => Some("web:White"),
        144 => Some("web:WhiteSmoke"),
        145 => Some("web:Yellow"),
        _ => None,
    }
}

pub fn style_system_color_name(code: i32) -> Option<&'static str> {
    match code {
        -1 => Some("style:FormBackColor"),
        -3 => Some("style:FormTextColor"),
        -11 => Some("style:FieldTextColor"),
        -15 => Some("style:ButtonTextColor"),
        // `uh` `StyleItems/ЦветФонаТолькоПросмотр` writes
        // `style:ButtonBackColor` for index `-7`; the form palette reads it the
        // same way on 68 occurrences.
        -7 => Some("style:ButtonBackColor"),
        -13 => Some("style:FieldTextColor"),
        // `-21` is `ButtonTextColor`, read off the platform's own `<Value>`
        // for `do` `StyleItems/ЦветКалендаряПодпись` and
        // `ЗаголовокКалендаряБроньЦветТекста`. The form palette
        // (`form_control_system_color_name`) reads the same code the same way
        // on 951 occurrences of the native form dumps of УТ 11.5.27.75, so the
        // two palettes agree here rather than differ. Every style-palette code
        // the style items of the stand exercise -- `-3`, `-16`, `-21`, `-23`,
        // `-42` -- agrees with the form palette; the codes where the two
        // tables still differ are exercised by neither.
        -21 => Some("style:ButtonTextColor"),
        -10 => Some("style:FieldSelectedTextColor"),
        -14 => Some("style:FieldAlternativeBackColor"),
        -23 => Some("style:ToolTipBackColor"),
        -24 => Some("style:ToolTipTextColor"),
        -16 => Some("style:SpecialTextColor"),
        -17 => Some("style:NegativeTextColor"),
        -22 => Some("style:BorderColor"),
        -25 => Some("style:ReportHeaderBackColor"),
        -26 => Some("style:ReportGroup1BackColor"),
        -27 => Some("style:ReportGroup2BackColor"),
        -28 => Some("style:ReportLineColor"),
        -34 => Some("style:ButtonBorderColor"),
        -35 => Some("style:TableHeaderBackColor"),
        -36 => Some("style:TableHeaderTextColor"),
        -37 => Some("style:TableFooterBackColor"),
        -38 => Some("style:TableFooterTextColor"),
        -42 => Some("style:NavigationColor"),
        -43 => Some("style:AuxiliaryNavigationColor"),
        -44 => Some("style:ActivityColor"),
        _ => None,
    }
}
