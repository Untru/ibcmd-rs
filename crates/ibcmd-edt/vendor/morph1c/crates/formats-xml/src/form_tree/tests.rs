//! Синтетические byte-exact тесты форм-рекурсии (foundation L1f proof). Строят
//! минимальную EDT-подобную форм-проекцию (контейнер `Group` с рекурсивными `items` +
//! лист `Label` с extInfo+событием) и доказывают:
//! 1. `read_items` строит правильное 2-уровневое дерево [`FormItem`];
//! 2. `write_items` регенерирует РОВНО исходные узлы (round-trip байт-в-байт);
//! 3. неразобранный под-узел контрола → ОШИБКА (§1.0, нет passthrough).
//!
//! Проекция здесь — НАМЕРЕННО маленькая (не реальный EDT/Designer): её цель — испытать
//! СУБСТРАТ рекурсии (form_tree.rs), а не закрыть корпус-форму. Реальные проекции
//! EDT/Designer — отдельный срез (см. отчёт foundation).

use super::*;
use crate::emit::{render, Envelope};
use crate::locus::{Codec, FieldProjection, XmlLocus};
use crate::{parse, LocusMap};
use morph1c_core::ir::value::{PropertyValue, Token, ValueKind};
use morph1c_core::ir::{FieldId, FormControlKind, FormEvent, FormItem};
use morph1c_core::spec::common::{EntitySpec, FieldSpec};
use morph1c_core::spec::forms::controls::ControlSpec;
use morph1c_core::version::SSL;
use std::sync::OnceLock;

// --- Спеки двух синтетических контролов ---

const F_TEXT: FieldId = FieldId(1); // общее свойство `<text>`
const F_ALIGN: FieldId = FieldId(101); // extInfo `<align>`
const F_UNITED: FieldId = FieldId(1); // Group общее свойство `<united>`

fn label_props_spec() -> &'static EntitySpec {
    static S: OnceLock<EntitySpec> = OnceLock::new();
    S.get_or_init(|| EntitySpec {
        entity: "Label",
        fields: Box::leak(
            vec![FieldSpec::with_default(
                F_TEXT,
                "text",
                ValueKind::Str,
                PropertyValue::Str(String::new()),
            )]
            .into_boxed_slice(),
        ),
        children: &[],
    })
}

fn label_ext_spec() -> &'static EntitySpec {
    static S: OnceLock<EntitySpec> = OnceLock::new();
    S.get_or_init(|| EntitySpec {
        entity: "LabelExtInfo",
        fields: Box::leak(
            vec![FieldSpec::with_default(
                F_ALIGN,
                "align",
                ValueKind::Enum,
                PropertyValue::Enum(Token::new("Left")),
            )]
            .into_boxed_slice(),
        ),
        children: &[],
    })
}

fn empty_spec(entity: &'static str) -> &'static EntitySpec {
    // Уникальный кэш на каждое имя сущности (extInfo Group пуст).
    static MAP: OnceLock<std::sync::Mutex<Vec<(&'static str, &'static EntitySpec)>>> =
        OnceLock::new();
    let m = MAP.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    let mut g = m.lock().unwrap();
    if let Some((_, s)) = g.iter().find(|(e, _)| *e == entity) {
        return s;
    }
    let s: &'static EntitySpec = Box::leak(Box::new(EntitySpec {
        entity,
        fields: &[],
        children: &[],
    }));
    g.push((entity, s));
    s
}

fn group_props_spec() -> &'static EntitySpec {
    static S: OnceLock<EntitySpec> = OnceLock::new();
    S.get_or_init(|| EntitySpec {
        entity: "Group",
        fields: Box::leak(
            vec![FieldSpec::with_default(
                F_UNITED,
                "united",
                ValueKind::Bool,
                PropertyValue::Bool(false),
            )]
            .into_boxed_slice(),
        ),
        children: &[],
    })
}

fn label_spec() -> &'static ControlSpec {
    static S: OnceLock<ControlSpec> = OnceLock::new();
    S.get_or_init(|| ControlSpec {
        kind: "Label",
        properties: label_props_spec(),
        ext_info: label_ext_spec(),
        container: false,
    })
}

fn group_spec() -> &'static ControlSpec {
    static S: OnceLock<ControlSpec> = OnceLock::new();
    S.get_or_init(|| ControlSpec {
        kind: "Group",
        properties: group_props_spec(),
        ext_info: empty_spec("GroupExtInfo"),
        container: true,
    })
}

// --- Проекции свойств/extInfo контролов (плоские теги без префикса) ---

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

struct LabelProps;
impl LocusMap for LabelProps {
    fn lookup(&self, f: FieldId) -> Option<FieldProjection> {
        (f == F_TEXT).then(|| FieldProjection::new(flat(&["text"]), Codec::PlainText))
    }
}
struct LabelExt;
impl LocusMap for LabelExt {
    fn lookup(&self, f: FieldId) -> Option<FieldProjection> {
        (f == F_ALIGN).then(|| FieldProjection::new(flat(&["align"]), Codec::EnumText))
    }
}
struct GroupProps;
impl LocusMap for GroupProps {
    fn lookup(&self, f: FieldId) -> Option<FieldProjection> {
        (f == F_UNITED).then(|| FieldProjection::new(flat(&["united"]), Codec::BoolPresence))
    }
}
struct EmptyMap;
impl LocusMap for EmptyMap {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}

// --- Синтетическая EDT-подобная форм-проекция ---
//
// Дискриминатор: `<items xsi:type="syn:<Kind>">`. Идентичность: дети `<name>`/`<id>`.
// extInfo: `<extInfo xsi:type="syn:<Kind>ExtInfo">`. События: `<handlers><event>/<name>`.
// Контейнер: дети — прямые `<items>` под узлом.

struct SynForm;

impl SynForm {
    fn bindings() -> &'static [ControlBinding] {
        static B: OnceLock<Vec<ControlBinding>> = OnceLock::new();
        B.get_or_init(|| {
            vec![
                ControlBinding {
                    kind: "Label",
                    spec: label_spec(),
                    property_map: &LabelProps,
                    ext_info_map: &LabelExt,
                },
                ControlBinding {
                    kind: "Group",
                    spec: group_spec(),
                    property_map: &GroupProps,
                    ext_info_map: &EmptyMap,
                },
            ]
        })
    }
}

/// Канонический вид из `xsi:type="syn:<Kind>"`.
fn kind_from_xsi(el: &Element) -> Option<FormControlKind> {
    let v = el.attr("xsi:type")?;
    v.value.strip_prefix("syn:").map(FormControlKind::new)
}

impl FormProjection for SynForm {
    fn version(&self) -> FormatVersion {
        SSL
    }
    fn emit_defaults(&self) -> bool {
        false
    }
    fn control_bindings(&self) -> &'static [ControlBinding] {
        Self::bindings()
    }

    fn read_item_nodes<'a>(
        &self,
        parent: &'a Element,
    ) -> Result<Vec<(&'a Element, FormControlKind)>, FormTreeError> {
        let mut out = Vec::new();
        for c in parent
            .children
            .iter()
            .filter(|c| c.local == "items" && c.prefix.is_empty())
        {
            let kind = kind_from_xsi(c).ok_or_else(|| {
                FormTreeError::Frame("missing/invalid xsi:type discriminator".into())
            })?;
            out.push((c, kind));
        }
        Ok(out)
    }

    fn read_item_identity(&self, el: &Element) -> Result<(String, i64), FormTreeError> {
        // claim xsi:type-атрибут дискриминатора (часть каркаса узла).
        if let Some(a) = el.attr("xsi:type") {
            a.claimed.set(true);
        }
        let name_el = el
            .child("name")
            .ok_or_else(|| FormTreeError::Frame("control missing <name>".into()))?;
        name_el.claim_with_text();
        let id_el = el
            .child("id")
            .ok_or_else(|| FormTreeError::Frame("control missing <id>".into()))?;
        id_el.claim_with_text();
        let id = id_el
            .text
            .parse::<i64>()
            .map_err(|e| FormTreeError::Frame(format!("bad <id>: {e}")))?;
        Ok((name_el.text.clone(), id))
    }

    fn read_item_props_root<'a>(&self, el: &'a Element) -> Result<&'a Element, FormTreeError> {
        Ok(el) // свойства — прямые дети узла.
    }

    fn read_item_events(&self, el: &Element) -> Result<Vec<FormEvent>, FormTreeError> {
        let mut out = Vec::new();
        for h in el
            .children
            .iter()
            .filter(|c| c.local == "handlers" && c.prefix.is_empty())
        {
            h.claim();
            let ev = h
                .child("event")
                .ok_or_else(|| FormTreeError::Frame("handlers: no <event>".into()))?;
            ev.claim_with_text();
            let nm = h
                .child("name")
                .ok_or_else(|| FormTreeError::Frame("handlers: no <name>".into()))?;
            nm.claim_with_text();
            out.push(FormEvent {
                name: ev.text.clone(),
                handler: nm.text.clone(),
            });
        }
        Ok(out)
    }

    fn read_item_ext_info_root<'a>(
        &self,
        el: &'a Element,
        _spec: &ControlSpec,
    ) -> Result<Option<&'a Element>, FormTreeError> {
        match el.child("extInfo") {
            Some(ext) => {
                ext.claim();
                if let Some(a) = ext.attr("xsi:type") {
                    a.claimed.set(true);
                }
                Ok(Some(ext))
            }
            None => Ok(None),
        }
    }

    fn build_item(&self, item: &FormItem, spec: &ControlSpec) -> Result<OutElement, FormTreeError> {
        let mut el = OutElement::branch("", "items").attr("xsi:type", format!("syn:{}", spec.kind));
        el.push(OutElement::leaf("", "name", item.name.clone()));
        el.push(OutElement::leaf("", "id", item.id.to_string()));
        Ok(el)
    }

    fn assemble_item(
        &self,
        el: &mut OutElement,
        item: &FormItem,
        spec: &ControlSpec,
        property_nodes: Vec<OutElement>,
        ext_info_nodes: Vec<OutElement>,
        child_nodes: Vec<OutElement>,
    ) -> Result<(), FormTreeError> {
        // Порядок эмиссии: свойства, события, extInfo, дети.
        for n in property_nodes {
            el.push(n);
        }
        for ev in &item.events {
            let mut h = OutElement::branch("", "handlers");
            h.push(OutElement::leaf("", "event", ev.name.clone()));
            h.push(OutElement::leaf("", "name", ev.handler.clone()));
            el.push(h);
        }
        if !ext_info_nodes.is_empty() {
            let mut ext = OutElement::branch("", "extInfo")
                .attr("xsi:type", format!("syn:{}ExtInfo", spec.kind));
            for n in ext_info_nodes {
                ext.push(n);
            }
            el.push(ext);
        }
        for c in child_nodes {
            el.push(c);
        }
        Ok(())
    }

    fn wrap_child_items(&self, children: Vec<OutElement>) -> Vec<OutElement> {
        children // EDT-подобно: дети — прямые `<items>` (без обёртки).
    }
}

fn env() -> Envelope {
    Envelope {
        bom: false,
        eol: "\n",
        indent_unit: "  ",
        decl: "<?xml version=\"1.0\"?>",
        trailing_eol: false,
        escape_gt: false,
        escape_quot: false,
        text_eol: "\n",
    }
}

/// Построить корень-форму, обернуть КОРНЕВОЙ список (тут — без обёртки) и срендерить.
fn render_form(items: &[FormItem]) -> Vec<u8> {
    let nodes = write_items(&SynForm, items).expect("write_items");
    let mut root = OutElement::branch("syn", "Form");
    for n in nodes {
        root.push(n);
    }
    render(&env(), &root)
}

#[test]
fn read_builds_two_level_tree() {
    let xml = "<?xml version=\"1.0\"?>\n\
<syn:Form>\n\
  <items xsi:type=\"syn:Group\">\n\
    <name>Группа</name>\n\
    <id>10</id>\n\
    <united>true</united>\n\
    <items xsi:type=\"syn:Label\">\n\
      <name>Надпись</name>\n\
      <id>11</id>\n\
      <text>Привет</text>\n\
      <handlers>\n\
        <event>OnClick</event>\n\
        <name>ПриНажатии</name>\n\
      </handlers>\n\
      <extInfo xsi:type=\"syn:LabelExtInfo\">\n\
        <align>Right</align>\n\
      </extInfo>\n\
    </items>\n\
  </items>\n\
</syn:Form>";
    let d = parse(xml.as_bytes()).expect("parse");
    d.root.claim();
    let items = read_items(&SynForm, &d.root).expect("read_items");
    assert_eq!(items.len(), 1, "one root control");
    let group = &items[0];
    assert_eq!(group.kind.as_str(), "Group");
    assert_eq!(group.name, "Группа");
    assert_eq!(group.id, 10);
    assert_eq!(
        group.properties,
        vec![(F_UNITED, PropertyValue::Bool(true))]
    );
    assert_eq!(group.children.len(), 1, "group has one child");
    let label = &group.children[0];
    assert_eq!(label.kind.as_str(), "Label");
    assert_eq!(label.id, 11);
    assert_eq!(
        label.properties,
        vec![(F_TEXT, PropertyValue::Str("Привет".into()))]
    );
    assert_eq!(
        label.ext_info,
        vec![(F_ALIGN, PropertyValue::Enum(Token::new("Right")))]
    );
    assert_eq!(
        label.events,
        vec![FormEvent {
            name: "OnClick".into(),
            handler: "ПриНажатии".into()
        }]
    );
    // §1.0: всё дерево консумировано.
    assert_eq!(d.root.unclaimed_count(), 0, "all nodes consumed");
}

#[test]
fn write_then_read_roundtrips_ir() {
    // Построить IR-дерево, записать, перечитать → равный IR (структурная регенерация).
    let mut label = FormItem::new(FormControlKind::new("Label"), "Надпись", 11);
    label.properties = vec![(F_TEXT, PropertyValue::Str("Привет".into()))];
    label.ext_info = vec![(F_ALIGN, PropertyValue::Enum(Token::new("Right")))];
    label.events = vec![FormEvent {
        name: "OnClick".into(),
        handler: "ПриНажатии".into(),
    }];
    let mut group = FormItem::new(FormControlKind::new("Group"), "Группа", 10);
    group.properties = vec![(F_UNITED, PropertyValue::Bool(true))];
    group.children = vec![label];

    let bytes = render_form(&[group.clone()]);
    let d = parse(&bytes).expect("re-parse");
    d.root.claim();
    let items = read_items(&SynForm, &d.root).expect("read");
    assert_eq!(
        items,
        vec![group],
        "write→read must round-trip the control tree IR"
    );
}

#[test]
fn write_is_byte_exact_stable() {
    // Дважды записать — байт-идентично (детерминизм эмиттера).
    let mut label = FormItem::new(FormControlKind::new("Label"), "L", 2);
    label.properties = vec![(F_TEXT, PropertyValue::Str("x".into()))];
    let mut group = FormItem::new(FormControlKind::new("Group"), "G", 1);
    group.properties = vec![(F_UNITED, PropertyValue::Bool(true))];
    group.children = vec![label];

    let a = render_form(&[group.clone()]);
    let b = render_form(&[group]);
    assert_eq!(a, b, "deterministic byte-exact emit");
    // И: re-parse → write снова даёт те же байты (round-trip byte-exact, §3.2).
    let d = parse(&a).expect("parse");
    d.root.claim();
    let items = read_items(&SynForm, &d.root).expect("read");
    let c = render_form(&items);
    assert_eq!(a, c, "render∘read∘render byte-exact");
}

#[test]
fn unconsumed_node_under_control_errors() {
    // Лишний тег <bogus> под контролом → ОШИБКА (§1.0, нет passthrough).
    let xml = "<?xml version=\"1.0\"?>\n\
<syn:Form>\n\
  <items xsi:type=\"syn:Label\">\n\
    <name>L</name>\n\
    <id>1</id>\n\
    <text>x</text>\n\
    <bogus>stray</bogus>\n\
  </items>\n\
</syn:Form>";
    let d = parse(xml.as_bytes()).expect("parse");
    d.root.claim();
    let res = read_items(&SynForm, &d.root);
    assert!(
        res.is_err(),
        "unconsumed <bogus> under control MUST error (§1.0), got {res:?}"
    );
}

#[test]
fn unknown_discriminator_errors() {
    // Неизвестный вид контрола → ОШИБКА (нет бинда), не silent-skip.
    let xml = "<?xml version=\"1.0\"?>\n\
<syn:Form>\n\
  <items xsi:type=\"syn:Mystery\">\n\
    <name>M</name>\n\
    <id>1</id>\n\
  </items>\n\
</syn:Form>";
    let d = parse(xml.as_bytes()).expect("parse");
    d.root.claim();
    let res = read_items(&SynForm, &d.root);
    assert!(res.is_err(), "unknown control kind MUST error, got {res:?}");
}
