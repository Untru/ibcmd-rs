//! FIELDS · shared codec helpers used by BOTH dialects (leaf/bool/int accessors, color canon↔text, picture canon, and the choiceList/choiceParameters/typeLink canon decomposers). No dialect-specific read/write logic here.

use super::*;

/// Claim ребёнка `prefix:local` и вернуть его текст (для нескольких-детей случаев Period).
pub(crate) fn child_leaf_text(
    el: &Element,
    prefix: &str,
    local: &str,
    tag: &str,
) -> Result<String, FormError> {
    let c = el
        .children
        .iter()
        .find(|c| c.local == local && c.prefix == prefix)
        .ok_or_else(|| {
            let disp = if prefix.is_empty() {
                local.to_string()
            } else {
                format!("{prefix}:{local}")
            };
            FormError::Frame(format!("<{tag}>: missing <{disp}> (§1.0)"))
        })?;
    c.claim_with_text();
    Ok(c.text.clone())
}

/// Канон рамки из пары (стиль, ширина): `Enum("Style")` при ширине 1 (подавляющий корпус:
/// 179/181 EDT), `Enum("Style@W")` при иной витнессированной ширине (`WithoutBorder@3` —
/// НастройкиРаботыСФайловымАрхивом, единственный не-1 witness обоих диалектов). Суффикс
/// НЕ равен keep-литералам политик (`Single`/`WithoutBorder`) ⇒ не-1-ширина всегда эмитится
/// обоими писателями.
pub(crate) fn border_canon(style: &str, width: &str) -> PropertyValue {
    if width == "1" {
        PropertyValue::Enum(Token::new(style))
    } else {
        PropertyValue::Enum(Token::new(format!("{style}@{width}")))
    }
}

/// Обратный разбор канона рамки: `"Style"` ⇒ (`Style`, `1`); `"Style@W"` ⇒ (`Style`, `W`).
pub(crate) fn border_split(canon: &str) -> (&str, &str) {
    match canon.split_once('@') {
        Some((style, width)) => (style, width),
        None => (canon, "1"),
    }
}

/// Разложить канон-период `List([Str(start), Str(end)])` в пару строк.
pub(crate) fn period_pair<'a>(
    value: &'a PropertyValue,
    tag: &str,
) -> Result<(&'a str, &'a str), FormError> {
    match value {
        PropertyValue::List(parts) if parts.len() == 2 => match (&parts[0], &parts[1]) {
            (PropertyValue::Str(s), PropertyValue::Str(e)) => Ok((s.as_str(), e.as_str())),
            _ => Err(FormError::Frame(format!(
                "<{tag}>: period parts must be Str (§1.6)"
            ))),
        },
        _ => Err(FormError::Frame(format!(
            "<{tag}>: period must be List[start,end] (§1.6)"
        ))),
    }
}

pub(crate) fn decode_bool_text(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    el.claim_with_text();
    match el.text.as_str() {
        "true" => Ok(PropertyValue::Bool(true)),
        "false" => Ok(PropertyValue::Bool(false)),
        other => Err(FormError::Frame(format!("<{tag}>={other:?}, want bool"))),
    }
}

pub(crate) fn bool_leaf(prefix: &str, tag: &str, b: bool) -> OutElement {
    OutElement::leaf(prefix, tag, if b { "true" } else { "false" })
}

/// Строковый литерал bool (`true`/`false`) для xml-текста.
pub(crate) fn bool_lit(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

pub(crate) fn parse_int(s: &str, tag: &str) -> Result<i64, FormError> {
    s.parse::<i64>()
        .map_err(|e| FormError::Frame(format!("<{tag}>: bad int {s:?}: {e}")))
}

/// Claim обязательного `xsi:type` со сверкой значения.
pub(crate) fn claim_xsi(el: &Element, tag: &str, want: &str) -> Result<(), FormError> {
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: missing xsi:type")))?;
    if xt.value != want {
        return Err(FormError::Frame(format!(
            "<{tag}> xsi:type={:?}, want {want:?}",
            xt.value
        )));
    }
    xt.claimed.set(true);
    Ok(())
}

/// Claim элемента + единственного ребёнка-листа `child`; вернуть его текст.
pub(crate) fn require_single_leaf(
    el: &Element,
    tag: &str,
    child: &str,
) -> Result<String, FormError> {
    el.claim();
    let c = el
        .child(child)
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: missing <{child}> (§1.0)")))?;
    c.claim_with_text();
    if el.children.len() != 1 {
        return Err(FormError::Frame(format!(
            "<{tag}>: expected exactly one <{child}>, found {} children (§1.0)",
            el.children.len()
        )));
    }
    Ok(c.text.clone())
}

/// Symbolic color references use the SDK's prefix mapping, independently of which
/// Windows names have appeared in a source corpus. A reference must name a target.
pub(crate) fn color_from_designer(text: &str, tag: &str) -> Result<String, FormError> {
    if let Some(rest) = text.strip_prefix("style:") {
        Ok(format!("Style.{rest}"))
    } else if let Some(rest) = text.strip_prefix("pal:") {
        Ok(format!("Palette.{rest}"))
    } else if let Some(rest) = text.strip_prefix("web:") {
        Ok(format!("Web.{rest}"))
    } else if let Some(rest) = text.strip_prefix("win:") {
        if rest.is_empty() {
            return Err(FormError::Frame(format!(
                "<{tag}>: empty Windows color reference"
            )));
        }
        Ok(format!("Windows.{rest}"))
    } else if text.starts_with('#') {
        // Web-hex литерал `#RRGGBB` (canon хранит его как есть; X с EDT ColorDef RGB-триплетом,
        // который декодится в тот же `#RRGGBB`). Witness — DataProcessor ПомощникСозданияОбменаДанными.
        if parse_hex_rgb(text).is_none() {
            return Err(FormError::Frame(format!(
                "<{tag}>={text:?}: malformed hex color, want #RRGGBB (§1.0)"
            )));
        }
        Ok(text.to_string())
    } else {
        Err(FormError::Frame(format!(
            "<{tag}>={text:?}: unmodeled color encoding (§1.0 — witnessed style:/pal:/web:/win:/#RRGGBB)"
        )))
    }
}

/// Designer-текст цвета ← каноническая кодировка.
pub(crate) fn color_to_designer(canon: &str, tag: &str) -> Result<String, FormError> {
    if let Some(rest) = canon.strip_prefix("Style.") {
        Ok(format!("style:{rest}"))
    } else if let Some(rest) = canon.strip_prefix("Palette.") {
        Ok(format!("pal:{rest}"))
    } else if let Some(rest) = canon.strip_prefix("Web.") {
        Ok(format!("web:{rest}"))
    } else if let Some(rest) = canon.strip_prefix("Windows.") {
        if rest.is_empty() {
            return Err(FormError::Frame(format!(
                "<{tag}>: empty Windows color reference"
            )));
        }
        Ok(format!("win:{rest}"))
    } else if canon.starts_with('#') {
        Ok(canon.to_string()) // Web-hex `#RRGGBB` — as-is.
    } else {
        Err(FormError::Frame(format!(
            "<{tag}>={canon:?}: unmodeled canonical color (§1.0 — witnessed Style./Palette./Web./Windows./#RRGGBB)"
        )))
    }
}

/// EDT-рендер цвета: канон `#RRGGBB` → `core:ColorDef` (RGB-триплет), иначе → `core:ColorRef`
/// (`<color>`-лист). Симметрично [`decode_edt_color`].
pub(crate) fn render_edt_color(tag: &str, canon: &str) -> Result<OutElement, FormError> {
    if canon.starts_with('#') {
        let (r, g, b) = parse_hex_rgb(canon).ok_or_else(|| {
            FormError::Frame(format!("EDT <{tag}>: malformed hex color {canon:?} (§1.6)"))
        })?;
        // EDT ОПУСКАЕТ нулевые компоненты; все-нулевой `#000000` — пустой самозакрытый
        // (зеркало `style_value_codec::encode_edt_color`; census ERP: 0 explicit `<c>0</c>`,
        // 649/1829 ColorDef самозакрыты). Порядок red/green/blue.
        if r == 0 && g == 0 && b == 0 {
            return Ok(OutElement::self_closing("", tag).attr("xsi:type", "core:ColorDef"));
        }
        let mut el = OutElement::branch("", tag).attr("xsi:type", "core:ColorDef");
        if r != 0 {
            el.push(OutElement::leaf("", "red", r.to_string()));
        }
        if g != 0 {
            el.push(OutElement::leaf("", "green", g.to_string()));
        }
        if b != 0 {
            el.push(OutElement::leaf("", "blue", b.to_string()));
        }
        Ok(el)
    } else {
        color_to_designer(canon, tag)?;
        let mut el = OutElement::branch("", tag).attr("xsi:type", "core:ColorRef");
        el.push(OutElement::leaf("", "color", canon.to_string()));
        Ok(el)
    }
}

/// Разобрать канон `#RRGGBB` (uppercase-hex) → `(r,g,b)`. `None` если не валидный hex-цвет.
fn parse_hex_rgb(canon: &str) -> Option<(u8, u8, u8)> {
    let hex = canon.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// `LoadTransparent`-ДЕФОЛТ, ВЫВОДИМЫЙ из вида ссылки (StdPicture.*⇒true, иначе false). EDT
/// picture-узел флаг НЕ несёт (census: 0/весь ERP-корпус) ⇒ на edt-read он ДЕРИВИТСЯ этой
/// функцией; Designer читает независимый флаг как есть. Также — fallback [`picture_ref_lt`]
/// для голого `Ref` (не-конвертированный источник).
pub(crate) fn picture_lt_default(picture_ref: &str) -> bool {
    super::super::picture_defaults::common_picture_default(picture_ref).unwrap_or_else(|| {
        picture_ref.starts_with(morph1c_core::spec::forms::command::PICTURE_STD_PREFIX)
    })
}

/// Разобрать текст `<xr:LoadTransparent>` в bool; иное — типизированный отказ (§1.0).
pub(crate) fn picture_lt_value(text: &str, tag: &str) -> Result<bool, FormError> {
    match text {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(FormError::Frame(format!(
            "<{tag}> <xr:LoadTransparent>={other:?}, want bool (§1.0)"
        ))),
    }
}

/// Канон картинки: `List([Ref(reference), Bool(loadTransparent)])`. `loadTransparent` —
/// НЕЗАВИСИМЫЙ витнессированный флаг (ERP census: LoadTransparent="true" при CommonPicture/Abs,
/// НЕ выводимый из вида ссылки), а не денормализация. `reference` — `"StdPicture.X"` /
/// `"CommonPicture.Y"` / `"abs:<ext>"` (сайдкар) / `""` (пустой form:FormPicture-маркер).
pub(crate) fn picture_canon(reference: String, load_transparent: bool) -> PropertyValue {
    PropertyValue::List(vec![
        PropertyValue::Ref(reference),
        PropertyValue::Bool(load_transparent),
    ])
}

/// Канон ВСТРОЕННОЙ картинки-СПУТНИКА с ПРОЗРАЧНЫМ ПИКСЕЛЕМ:
/// `List([Ref(reference), Bool(true), List([Int(x), Int(y)])])` — ТРЕТИЙ элемент несёт пиксель.
///
/// Пиксель встречается ТОЛЬКО у спутниковой (`""`/`"abs:<ext>"`) картинки и ДЕНОРМАЛИЗУЕТ
/// `loadTransparent=true` (census ERP форм 100/100: `<xr:Abs>`+LT=true ⟺ `<xr:TransparentPixel>`,
/// а `<xr:Ref>`-метассылки пикселя не несут). EDT-носитель — непустой `<tag
/// xsi:type="form:FormPicture"><transparentPixel><x/><y/></transparentPixel></tag>` (sparse-листья);
/// Designer-носитель — третий ребёнок `<xr:TransparentPixel x=.. y=../>` (DENSE-атрибуты). Оба
/// диалекта дают РАВНЫЙ IR (§1.6).
pub(crate) fn picture_canon_px(reference: String, pixel: (i64, i64)) -> PropertyValue {
    PropertyValue::List(vec![
        PropertyValue::Ref(reference),
        PropertyValue::Bool(true),
        PropertyValue::List(vec![
            PropertyValue::Int(pixel.0),
            PropertyValue::Int(pixel.1),
        ]),
    ])
}

/// Прозрачный пиксель картинки, если канон его несёт (третий элемент `List([Int(x), Int(y)])`
/// у [`picture_canon_px`]). `None` — обычная 2-элементная картинка (без встроенного пикселя).
pub(crate) fn picture_pixel(v: &PropertyValue) -> Option<(i64, i64)> {
    if let PropertyValue::List(p) = v {
        if let Some(PropertyValue::List(px)) = p.get(2) {
            if let [PropertyValue::Int(x), PropertyValue::Int(y)] = px.as_slice() {
                return Some((*x, *y));
            }
        }
    }
    None
}

/// Разложить канон картинки в `(reference, loadTransparent)`. Принимает канон `List([Ref, Bool])`,
/// расширенный канон `List([Ref, Bool, Pixel])` ([`picture_canon_px`] — пиксель извлекается
/// отдельно [`picture_pixel`]) И голый `Ref` (тогда `loadTransparent` ДЕРИВИТСЯ
/// [`picture_lt_default`] — путь совместимости для не-конвертированных источников; §1.0-дефолт,
/// не догадка).
pub(crate) fn picture_ref_lt(v: &PropertyValue) -> Result<(&str, bool), FormError> {
    match v {
        PropertyValue::List(p) if p.len() == 4 => {
            let (PropertyValue::Ref(reference), PropertyValue::Bool(flag)) = (&p[0], &p[1]) else { return Err(FormError::Frame("malformed picture definition".into())); };
            if !reference.is_empty() && !reference.starts_with("abs-file:") { return Err(FormError::Frame("glyph requires a picture definition".into())); }
            let point = PropertyValue::List(vec![p[2].clone(),p[3].clone()]);
            crate::md_picture::point(&point).map_err(FormError::Frame)?;
            crate::md_picture::glyph(&point).map_err(FormError::Frame)?;
            Ok((reference, *flag))
        },
        PropertyValue::List(p) if p.len() == 2 || p.len() == 3 => match (&p[0], &p[1]) {
            (PropertyValue::Ref(r), PropertyValue::Bool(lt)) => Ok((r.as_str(), *lt)),
            _ => Err(FormError::Frame(
                "picture canon must be List([Ref, Bool[, Pixel]]) (§1.0)".into(),
            )),
        },
        PropertyValue::Ref(r) => Ok((r.as_str(), picture_lt_default(r))),
        other => Err(FormError::Frame(format!(
            "picture value must be Ref|List([Ref,Bool[,Pixel]]), got {:?} (§1.0)",
            other.kind()
        ))),
    }
}

// ============================ choiceList (ValueList) ============================
//
// Канон одного пункта — `PropertyValue::List([presentation:Localized, value:Value])`;
// весь список — `PropertyValue::List([пункт, …])`. EDT: повторяемый `<choiceList>`
// (`<presentation>` key/value + `<value xsi:type="core:…Value">`). Designer: один
// `<ChoiceList>` c `<xr:Item>` (каркас `<xr:Presentation/>` пустой + `<xr:CheckState>0` +
// `<xr:Value xsi:type="FormChoiceListDesTimeValue">` c вложенными `<Presentation>` v8:item и
// `<Value xsi:type="xs:…">`). Скалярное значение — через общий [`value_codec`].

/// Разложить пункт в `(presentation, value, опц. picture)`. Пункт — `List([presentation, value])`
/// (без картинки) ЛИБО `List([presentation, value, picture:Ref])` (с картинкой — RadioButtonField
/// `СохранениеПечатнойФормы`). Картинка — [`PropertyValue::Ref`] (`CommonPicture.*` через PictureRef).
pub(crate) fn choice_item(
    item: &PropertyValue,
) -> Result<(&PropertyValue, &PropertyValue, Option<&PropertyValue>), FormError> {
    match item {
        PropertyValue::List(p) if p.len() == 2 => Ok((&p[0], &p[1], None)),
        PropertyValue::List(p) if p.len() == 3 => Ok((&p[0], &p[1], Some(&p[2]))),
        _ => Err(FormError::Frame(
            "choiceList item must be [presentation, value(, picture)] (§1.0)".into(),
        )),
    }
}

// ============================ choiceParameters (FormChoiceListDesTimeValue) ============================
//
// Форм-поле `choiceParameters`/`ChoiceParameters` — параметры выбора поля-ввода (name + значение
// дизайн-тайм). ОТЛИЧАЕТСЯ от дескрипторного `choiceParameters` ([`crate::choice_parameters`]) тем,
// что значение обёрнуто в `FormChoiceListDesTimeValue` (пустой `<Presentation/>` + скаляр `<Value>`),
// а не является прямым скаляром/`FixedArray`. Расхождение форматов:
// * EDT: повторяемый `<choiceParameters><name>Path</name><value xsi:type="form:FormChoiceListDesTimeValue">
//   <value xsi:type="core:…Value">…</value></value></choiceParameters>` (Presentation опущен — пуст).
// * Designer: контейнер `<ChoiceParameters><app:item name="Path"><app:value
//   xsi:type="FormChoiceListDesTimeValue"><Presentation/><Value xsi:type="xs:…">…</Value></app:value>
//   </app:item></ChoiceParameters>` (Presentation — пустой self-close).
// Канон одного пункта — `List([name:Str, value:Value])`; весь список — `List([пункт, …])`. Пустой
// Presentation реконструируется пер-диалектно (EDT опускает, Designer self-close), в IR не хранится.
// Current nonempty wrapper metadata is [Localized, value, optional picture];
// a null picture and empty presentation retain the legacy plain value shape.

/// EDT/Designer xsi:type обёртки значения параметра выбора.
pub(crate) const FCLDTV_EDT: &str = "form:FormChoiceListDesTimeValue";
pub(crate) const FCLDTV_DES: &str = "FormChoiceListDesTimeValue";

/// Значение пункта choiceParameters: скаляр (`Value`, обёрнут в FormChoiceListDesTimeValue)
/// либо МАССИВ (`List(Value…)`, FixedArray — ERP-witness ТипыДействийЭтаповПодготовкиБюджетов)
/// либо БЕЗ-ОБЁРТОЧНЫЙ `Undefined` (пустой выбор — прямой `core:UndefinedValue`/`xsi:nil`, НЕ
/// завёрнутый; ERP-witness ×7: РеквизитыОрганизации/ВыплатаЗарплаты/…). Без-обёрточный
/// `Undefined` — ОТДЕЛЬНАЯ байт-форма от завёрнутого-Undefined (×1 РесурсныеСпецификации:
/// `FCLDTV`+внутренний `core:UndefinedValue`); обе обязаны round-trip'иться байт-точно, поэтому
/// различаются в IR арностью пункта (`List([name])` — без-обёрточный; `List([name, value])` —
/// завёрнутый).
pub(crate) enum ChoiceParamValue<'a> {
    /// Current wrapper metadata: [Localized, current value, optional picture].
    Wrapped(&'a PropertyValue),
    /// Без-обёрточный `Undefined` (пустой выбор): EDT `<value xsi:type="core:UndefinedValue"/>`
    /// ⟺ Designer `<app:value xsi:nil="true"/>`. cf хранит его ПЛОСКОЙ `{"U"}`-ячейкой БЕЗ
    /// value-list-item-обёртки (эталон erp.cf ×7: `{0,1,"ПоОстаткам",{"U"}}` и т.п.) — в отличие
    /// от завёрнутого-Undefined (арность-2), который несёт item `{"#",0e704aa2-…,{…{"U"}…}}`.
    BareUndefined,
}

/// Разложить пункт choiceParameters в `(name, value)`. Пункт — `List([name:Str, value])`,
/// где value = `Value(скаляр)` либо `List(Value…)` (FixedArray).
pub(crate) fn choice_param_item(
    item: &PropertyValue,
) -> Result<(&str, ChoiceParamValue<'_>), FormError> {
    match item {
        // Арность 1 = БЕЗ-ОБЁРТОЧНЫЙ Undefined (пустой выбор); значение в IR не хранится.
        PropertyValue::List(p) if p.len() == 1 => match &p[0] {
            PropertyValue::Str(name) => Ok((name.as_str(), ChoiceParamValue::BareUndefined)),
            _ => Err(FormError::Frame(
                "choiceParameters bare item must be [name:Str] (§1.0)".into(),
            )),
        },
        PropertyValue::List(p) if p.len() == 2 => match (&p[0], &p[1]) {
            (PropertyValue::Str(name), value @ PropertyValue::Value(_)) => {
                Ok((name.as_str(), ChoiceParamValue::Wrapped(value)))
            }
            (PropertyValue::Str(name), value @ PropertyValue::List(arr))
                if matches!(arr.first(), Some(PropertyValue::Localized(_))) => {
                choice_wrapper_parts(value)?;
                Ok((name.as_str(), ChoiceParamValue::Wrapped(value)))
            }
            (PropertyValue::Str(name), value @ PropertyValue::List(_)) => {
                Ok((name.as_str(), ChoiceParamValue::Wrapped(value)))
            }
            _ => Err(FormError::Frame(
                "choiceParameters item must be [name:Str, value:Value|List] (§1.6)".into(),
            )),
        },
        _ => Err(FormError::Frame(
            "choiceParameters item must be [name] | [name, value] (§1.0)".into(),
        )),
    }
}

/// Wrapper metadata is authoritative typed data, never a lexical source copy.
/// Plain legacy values denote empty presentation and a null picture.
pub(crate) fn choice_wrapper_parts(value: &PropertyValue)
    -> Result<(&PropertyValue, &[(Lang, String)], Option<&PropertyValue>), FormError> {
    if let PropertyValue::List(parts) = value {
        if matches!(parts.first(), Some(PropertyValue::Localized(_))) {
            if parts.len() != 2 && parts.len() != 3 {
                return Err(FormError::Frame("choice wrapper metadata arity".into()));
            }
            let PropertyValue::Localized(presentation) = &parts[0] else { unreachable!() };
            if let Some(picture) = parts.get(2) { picture_ref_lt(picture)?; }
            return Ok((&parts[1], presentation, parts.get(2)));
        }
    }
    Ok((value, &[], None))
}
pub(crate) fn choice_wrapper_value(value: PropertyValue, presentation: Vec<(Lang, String)>, picture: Option<PropertyValue>) -> PropertyValue {
    if presentation.is_empty() && picture.is_none() { return value; }
    let mut parts = vec![PropertyValue::Localized(presentation), value];
    if let Some(picture) = picture { parts.push(picture); }
    PropertyValue::List(parts)
}
pub(crate) fn choice_picture_projection() -> FieldProj {
    fp(FieldId(0), "picture", "Picture", Region::Ext, Codec::PictureRef, Policy::Symmetric)
}

/// Канонический `Undefined`-скаляр для без-обёрточного пустого выбора choiceParameters.
pub(crate) fn bare_undefined_spec() -> morph1c_core::ir::value::ValueSpec {
    morph1c_core::ir::value::ValueSpec {
        kind: morph1c_core::ir::value::ValueScalarKind::Undefined,
        scalar: None,
    }
}

// ============================ typeLink / choiceParameterLinks ============================
//
// Structured-связи поля-ввода (метамодель InputFieldExtInfo: choiceForm 47 → choiceParameterLinks 48
// → choiceParameters 49; typeLink 63 — перед heightControlVariant). Витнессы SSL:
// * typeLink: EDT `<typeLink>[<linkItem>1</linkItem>]<datapath xsi:type="form:DataPath">
//   <segments>Items.X.CurrentData.Y</segments></datapath></typeLink>` (linkItem ОМИТ при 0; 6×0/1×1)
//   ⟺ Designer `<TypeLink><DataPath>Items.X.CurrentData.Y</DataPath><LinkItem>N</LinkItem></TypeLink>`
//   (оба листа ВСЕГДА; DataPath БЕЗ xsi:type; порядок обратный EDT).
// * choiceParameterLinks: EDT повторяемый `<choiceParameterLinks><name>Отбор.Владелец</name>
//   <datapath xsi:type="form:DataPath"><segments>Путь</segments></datapath></choiceParameterLinks>`
//   ⟺ Designer контейнер `<ChoiceParameterLinks><Link><Name>…</Name><DataPath xsi:type="xs:string">
//   Путь</DataPath><ValueChange>Clear</ValueChange></Link>…</ChoiceParameterLinks>`. `ValueChange`
//   witnessed ТОЛЬКО `Clear` (дефолт метамодели, EDT его опускает) — Designer-каркас: сверяется
//   на read (§1.0 при ином), реконструируется на write; в канон НЕ входит.
// ВНИМАНИЕ: EDT-тег вложенного пути — СТРОЧНЫЙ `datapath` (не `dataPath` Codec::DataPath).

/// Разложить канон typeLink в `(путь, linkItem)`.
pub(crate) fn type_link_parts(v: &PropertyValue) -> Result<(&str, i64), FormError> {
    match v {
        PropertyValue::List(p) if p.len() == 2 => match (&p[0], &p[1]) {
            (PropertyValue::Ref(path), PropertyValue::Int(n)) => Ok((path.as_str(), *n)),
            _ => Err(FormError::Frame(
                "typeLink canon must be [Ref, Int] (§1.6)".into(),
            )),
        },
        _ => Err(FormError::Frame(
            "typeLink canon must be [path, linkItem] (§1.6)".into(),
        )),
    }
}

/// Разложить пункт choiceParameterLinks в `(name, path)`.
/// Части пункта связи: `[name, path]` (Clear, SSL-канон) либо `[name, path, changeMode]`
/// (ERP-witness ЧОА Международный.ФормаСчета: `DontChange`; третий элемент SPARSE — только
/// НЕ-Clear, как в дескрипторном кодеке `choice_param_links`).
pub(crate) fn choice_parameter_link_parts(
    item: &PropertyValue,
) -> Result<(&str, &str, Option<&str>), FormError> {
    match item {
        PropertyValue::List(p) if p.len() == 2 || p.len() == 3 => {
            let mode = match p.get(2) {
                None => None,
                Some(PropertyValue::Enum(m)) => Some(m.as_str()),
                Some(other) => {
                    return Err(FormError::Frame(format!(
                        "choiceParameterLinks item[2] must be Enum(changeMode), got {other:?} (§1.6)"
                    )));
                }
            };
            match (&p[0], &p[1]) {
                (PropertyValue::Str(name), PropertyValue::Ref(path)) => {
                    Ok((name.as_str(), path.as_str(), mode))
                }
                _ => Err(FormError::Frame(
                    "choiceParameterLinks item must be [name:Str, path:Ref, changeMode?] (§1.6)"
                        .into(),
                )),
            }
        }
        _ => Err(FormError::Frame(
            "choiceParameterLinks item must be [name, path, changeMode?] (§1.0)".into(),
        )),
    }
}

pub(crate) fn picture_glyph(value: &PropertyValue) -> Result<Option<(i64, i64)>, FormError> {
    if let PropertyValue::List(parts) = value {
        if parts.len() == 4 {
            picture_ref_lt(value)?;
            return crate::md_picture::glyph(&PropertyValue::List(vec![
                parts[2].clone(),
                parts[3].clone(),
            ]))
            .map_err(FormError::Frame);
        }
    }
    Ok(None)
}
pub(crate) fn picture_with_glyph(
    value: PropertyValue,
    glyph: Option<(i64, i64)>,
) -> Result<PropertyValue, FormError> {
    let Some(glyph) = glyph else {
        let (reference, flag) = picture_ref_lt(&value)?;
        return Ok(match picture_pixel(&value) {
            Some(point) => picture_canon_px(reference.into(), point),
            None => picture_canon(reference.into(), flag),
        });
    };
    let (reference, flag) = picture_ref_lt(&value)?;
    let point = picture_pixel(&value);
    let PropertyValue::List(mut points) = crate::md_picture::present_with_glyph(point, Some(glyph))
    else {
        unreachable!()
    };
    let out = PropertyValue::List(vec![
        PropertyValue::Ref(reference.into()),
        PropertyValue::Bool(flag),
        points.remove(0),
        points.remove(0),
    ]);
    picture_ref_lt(&out)?;
    Ok(out)
}
