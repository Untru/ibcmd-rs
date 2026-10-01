//! ИНЛАЙН-картинки контролов формы: какие picture-СВОЙСТВА какого вида контрола могут нести
//! файл-СПУТНИК, и как этот спутник называется.
//!
//! # Зачем отдельный модуль (и почему ЗДЕСЬ, а не в pipeline)
//! Байты картинки лежат в спутнике `Items/<ctl>/<Tag>.<ext>` — их читает/пишет pipeline
//! (у XML-ридера на входе только байты дескриптора, каталога он не видит; ровно та же
//! развязка, что у `SpreadsheetData.mxlx` / `ListSettings.dcss`). Но ЗНАНИЕ о том, какие
//! свойства бывают картинками и как называется их тег, живёт в проекционных таблицах
//! [`super::tables`] — и дублировать его в pipeline нельзя: `FieldId` ПЕР-ВИДОВЫЕ
//! (`button::F_PICTURE == form_field::F_HEADER_PICTURE == FieldId(25)`), поэтому вне
//! контекста вида контрола `FieldId` НЕ адресует свойство. Этот модуль — единственный мост:
//! отдаёт слоты, разрешённые по ТЕМ ЖЕ таблицам, что использует ридер/райтер.
//!
//! # Имя спутника = Designer-тег (сверено 19/19 SSL)
//! `Picture` / `RowsPicture` / `ValuesPicture` / `HeaderPicture` (+ незасвидетельствованный в
//! SSL `ChoiceButtonPicture`). Основа имени файла ВСЕГДА равна Designer-тегу свойства,
//! расширение (`zip`/`png`/`svg`) несёт тип картинки. Внутри ОДНОГО вида контрола два
//! picture-свойства никогда не делят тег ⇒ тег однозначно адресует свойство.
//!
//! # Канон значения
//! * `Ref("<Kind>.<Name>")` — ссылка на метаданные (`StdPicture.*`/`CommonPicture.*`): спутника
//!   НЕТ.
//! * `Ref("abs:<ext>")` — картинка-СПУТНИК: байты в [`morph1c_core::ir::form::FormPicture`].
//!   Designer несёт `<xr:Abs><Tag>.<ext></xr:Abs>` ⇒ расширение известно из дескриптора; EDT
//!   несёт лишь пустой маркер `<tag xsi:type="form:FormPicture"/>` ⇒ расширение известно ТОЛЬКО
//!   от файла на диске, и pipeline ДОПИСЫВАЕТ канон до `abs:<ext>` при attach'е (до attach'а
//!   EDT-канон — `Ref("")`, «спутник, расширение неизвестно»).

use morph1c_core::ir::form::{FormControlKind, FormItem};
use morph1c_core::ir::{FieldId, PropertyValue};

use super::fields::{Codec, Region};
use super::tables;

/// Хвост канона picture-значения, означающий «картинка лежит в СПУТНИКЕ».
const ABS: &str = "abs:";

/// Один picture-СЛОТ вида контрола — свойство, которое МОЖЕТ нести спутник.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PictureSlot {
    /// Канонический id свойства (ПЕР-ВИДОВОЙ — осмыслен только вместе с видом контрола).
    pub id: FieldId,
    /// `true` ⇒ значение лежит в [`FormItem::ext_info`]; `false` ⇒ в [`FormItem::properties`].
    pub ext: bool,
    /// Основа имени файла-спутника = Designer-тег свойства.
    pub stem: &'static str,
}

/// Собрать picture-слоты из проекционной таблицы (`ext` — из какого бага её поля читаются).
fn slots_of(table: &'static [super::fields::FieldProj], out: &mut Vec<PictureSlot>) {
    for e in table {
        if matches!(e.codec, Codec::PictureRef) {
            out.push(PictureSlot {
                id: e.id,
                // В EDT-таблице регион поля и есть его баг: Region::Ext ⇒ extInfo.
                ext: matches!(e.region, Region::Ext),
                stem: e.des,
            });
        }
    }
}

/// ВСЕ picture-слоты вида контрола `kind` (тело + extInfo), разрешённые по тем же таблицам,
/// что использует ридер/райтер. Пусто у видов без picture-свойств.
pub fn picture_slots(kind: &FormControlKind) -> Vec<PictureSlot> {
    let k = kind.as_str();
    let mut out = Vec::new();
    if let Some(fk) = tables::field_kind(k) {
        slots_of(tables::FORM_FIELD_COMMON, &mut out);
        slots_of(fk.ext, &mut out);
    } else if let Some(gk) = tables::group_kind(k) {
        slots_of(tables::FORM_GROUP_BODY, &mut out);
        slots_of(gk.ext, &mut out);
    } else if let Some(dk) = tables::decoration_kind(k) {
        slots_of(tables::DECORATION_BODY, &mut out);
        slots_of(dk.ext, &mut out);
    } else if k == "Button" {
        slots_of(tables::BUTTON_BODY, &mut out);
    } else if k == "Table" {
        slots_of(tables::TABLE_BODY, &mut out);
    }
    out
}

/// Choice-list picture slots from the same closed field projection table.
pub(crate) fn choice_picture_slots(kind: &FormControlKind) -> Vec<PictureSlot> {
    let mut out = Vec::new();
    if let Some(field) = tables::field_kind(kind.as_str()) {
        for projection in field.ext {
            if matches!(projection.codec, Codec::ChoiceList) {
                out.push(PictureSlot {
                    id: projection.id,
                    ext: matches!(projection.region, Region::Ext),
                    stem: projection.des,
                });
            }
        }
    }
    out
}

/// Значение слота внутри контрола (из нужного бага).
fn slot_value<'a>(item: &'a FormItem, s: &PictureSlot) -> Option<&'a PropertyValue> {
    let bag = if s.ext {
        &item.ext_info
    } else {
        &item.properties
    };
    bag.iter().find(|(k, _)| *k == s.id).map(|(_, v)| v)
}

/// `true` ⇒ значение — картинка-СПУТНИК: EDT-маркер `Ref("")` либо Designer `Ref("abs:<ext>")`.
/// Ссылка на метаданные (`StdPicture.*`/`CommonPicture.*`) — `false` (спутника нет). Канон —
/// `List([Ref, Bool(loadTransparent)])` (W25); флаг для sidecar-детекции несуществен —
/// адресует спутник ТОЛЬКО ссылочная часть.
fn is_sidecar(v: &PropertyValue) -> bool {
    matches!(
        super::fields::picture_ref_lt(v),
        Ok((r, _lt)) if r.is_empty() || r.starts_with(ABS)
    )
}

/// Каждый picture-слот контрола, чьё значение — СПУТНИК, вместе с расширением, если дескриптор
/// его несёт (Designer — да; EDT — `None`, расширение узнаётся только от файла на диске).
///
/// Это ЕДИНСТВЕННЫЙ вход для pipeline: он говорит «этот контрол ждёт спутник с такой основой
/// имени» — а найти/прочитать/записать файл уже дело pipeline'а.
pub fn sidecar_slots(item: &FormItem) -> Vec<(PictureSlot, Option<String>)> {
    picture_slots(&item.kind)
        .into_iter()
        .filter_map(|s| {
            let v = slot_value(item, &s)?;
            if !is_sidecar(v) {
                return None;
            }
            let (r, _lt) = super::fields::picture_ref_lt(v).ok()?;
            Some((s, r.strip_prefix(ABS).map(str::to_string)))
        })
        .collect()
}

/// Дописать канон слота до `List([Ref("abs:<ext>"), Bool(lt)])` (EDT: после attach'а расширение
/// стало известно от файла — до этого канон был пустым маркером `List([Ref(""), Bool(lt)])`).
/// LoadTransparent СОХРАНЯЕТСЯ (W25 — независимый флаг). Идемпотентно: Designer-канон уже такой,
/// и повторная запись его не меняет ⇒ ОБА диалекта дают РАВНЫЙ IR (§1.6, с точностью до
/// EDT-неспособности нести LoadTransparent).
pub fn set_sidecar_ext(item: &mut FormItem, s: &PictureSlot, ext: &str) {
    let bag = if s.ext {
        &mut item.ext_info
    } else {
        &mut item.properties
    };
    if let Some((_, v)) = bag.iter_mut().find(|(k, _)| *k == s.id) {
        // ВСТРОЕННЫЙ прозрачный пиксель (третий элемент канона) СОХРАНЯЕТСЯ через attach —
        // иначе EDT-сторона (пиксель в дескрипторе) потеряла бы его. LT при пикселе — true.
        match super::fields::picture_pixel(v) {
            Some(px) => *v = super::fields::picture_canon_px(format!("{ABS}{ext}"), px),
            None => {
                let lt = super::fields::picture_ref_lt(v)
                    .map(|(_, lt)| lt)
                    .unwrap_or(false);
                *v = super::fields::picture_canon(format!("{ABS}{ext}"), lt);
            }
        }
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    #[test]
    fn button_picture_slot_is_body_keyed_picture() {
        let s = picture_slots(&FormControlKind::new("Button"));
        assert_eq!(s.len(), 1, "Button carries exactly one picture property");
        assert_eq!(s[0].stem, "Picture");
        assert!(!s[0].ext, "Button.picture lives in the BODY bag");
    }

    #[test]
    fn table_carries_rows_picture() {
        let s = picture_slots(&FormControlKind::new("Table"));
        assert!(s.iter().any(|p| p.stem == "RowsPicture" && !p.ext));
    }

    #[test]
    fn input_field_carries_header_body_and_ext_pictures() {
        let s = picture_slots(&FormControlKind::new("InputField"));
        // headerPicture — общее тело поля; picture/choiceButtonPicture — extInfo InputField.
        assert!(s.iter().any(|p| p.stem == "HeaderPicture" && !p.ext));
        assert!(s.iter().any(|p| p.stem == "Picture" && p.ext));
        assert!(s.iter().any(|p| p.stem == "ChoiceButtonPicture" && p.ext));
    }

    #[test]
    fn picture_field_carries_values_picture() {
        let s = picture_slots(&FormControlKind::new("PictureField"));
        assert!(s.iter().any(|p| p.stem == "ValuesPicture" && p.ext));
    }

    /// Внутри ОДНОГО вида два picture-свойства не делят тег — иначе имя файла-спутника не
    /// адресовало бы свойство однозначно (инвариант, на котором держится `FormItem::picture`).
    #[test]
    fn stems_are_unique_within_every_control_kind() {
        for k in tables::FIELD_KINDS
            .iter()
            .map(|f| f.kind)
            .chain(tables::GROUP_KINDS.iter().map(|g| g.kind))
            .chain(tables::DECORATION_KINDS.iter().map(|d| d.kind))
            .chain(["Button", "Table"])
        {
            let slots = picture_slots(&FormControlKind::new(k));
            let mut stems: Vec<_> = slots.iter().map(|s| s.stem).collect();
            let n = stems.len();
            stems.sort_unstable();
            stems.dedup();
            assert_eq!(stems.len(), n, "{k}: duplicate picture stem {stems:?}");
        }
    }

    #[test]
    fn metadata_ref_is_not_a_sidecar() {
        assert!(!is_sidecar(&PropertyValue::Ref(
            "StdPicture.ExchangePlan".into()
        )));
        assert!(!is_sidecar(&PropertyValue::Ref("CommonPicture.X".into())));
        assert!(is_sidecar(&PropertyValue::Ref(String::new())), "EDT marker");
        assert!(
            is_sidecar(&PropertyValue::Ref("abs:zip".into())),
            "Designer"
        );
    }

    /// W25-канон `List([Ref, Bool])` детектируется по ссылочной части (флаг несуществен).
    #[test]
    fn list_canon_sidecar_detection_ignores_load_transparent() {
        let canon = |r: &str, lt: bool| {
            PropertyValue::List(vec![PropertyValue::Ref(r.into()), PropertyValue::Bool(lt)])
        };
        assert!(is_sidecar(&canon("abs:png", true)), "Designer Abs LT=true");
        assert!(is_sidecar(&canon("", false)), "EDT marker");
        assert!(!is_sidecar(&canon("CommonPicture.X", true)));
        assert!(!is_sidecar(&canon("StdPicture.Refresh", true)));
    }
}
