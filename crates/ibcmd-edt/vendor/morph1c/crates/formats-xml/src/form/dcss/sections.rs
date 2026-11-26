//! ТРЕТИЙ ЛОКУС тех же настроек: cf-СЕКЦИИ бэга динсписка (`{"#",<type-guid>,{#base64:<XML>}}`).

use super::*;

// ─────────────────────────────────────────────────────────────────────────────────────────
// ТРЕТИЙ ЛОКУС ТЕХ ЖЕ НАСТРОЕК: cf-СЕКЦИИ БЭГА ДИНСПИСКА
// ─────────────────────────────────────────────────────────────────────────────────────────

/// СЕКЦИЯ настроек динсписка в cf-бэге (`{"#",<type-guid>,{#base64:<XML>}}`).
///
/// # Что это (RE F-wave 14, 58 несущих динсписков SSL / 77 секций)
/// Настройки списка живут в ТРЁХ локусах: Designer-инлайн `<ListSettings>`, EDT-сайдкар
/// `ListSettings.dcss` (оба — цельный `<Settings>`-документ, см. модульный docstring) и — в `.cf`
/// — РАЗОБРАННЫМИ ПО СЕКЦИЯМ: каждая часть настроек (`order`/`filter`/`conditionalAppearance`/
/// структурные группировки) переоформляется в СВОЙ 3-ns XML-документ и кладётся в бэг под своим
/// ключом, base64. Тип-guid КОНСТАНТЕН на секцию (сверено по всем 77 витнессам, 0 расхождений).
///
/// # Когда секция появляется
/// РОВНО тогда, когда ЕЁ часть отклоняется от платформенного дефолта
/// ([`DcsSettingsSection::deviates`]) — посекционно, а не «настройки в целом отклоняются»:
/// witness `Catalog.СценарииОбменовДанными.НастройкаРасписанияОбменовДанными` несёт
/// НЕ-дефолтные `dataParameters`, но в бэге у него ТОЛЬКО `Order` (dataParameters секции не
/// порождают — они целиком живут в отдельной ячейке `DataParameters`). Отсутствие сайдкара
/// (`<ListSettings/>`, ×2) = отклонение ВСЕХ ЧЕТЫРЁХ ⇒ четыре ПУСТЫЕ секции.
///
/// # ns-конвенция вывода (сериализатор платформы, воспроизведена byte-exact)
/// Корень объявляет РОВНО три ns (settings-по-умолчанию + `xs` + `xsi`) — вместо 11 сайдкара.
/// Любой ДРУГОЙ ns объявляется ИНЛАЙН на том элементе, где впервые понадобился, и живёт только
/// в его поддереве (сиблинги `<left>`/`<right>` объявляют `xmlns:dcscor` КАЖДЫЙ свой). Префикс:
/// * ns самой DCS (`…/settings`, `…/core`) — КАНОНИЧЕСКИЙ (`dcsset`, `dcscor`);
/// * любой другой (`8.1/data/core`, `8.1/data/ui`, `8.2/data/types`) — АВТО-префикс
///   `d<глубина>p<номер>` (глубина корня = 1; номер — по порядку авто-объявлений НА ЭТОМ
///   элементе): `<right xmlns:d3p1="…8.1/data/core" xmlns:d3p2="…8.2/data/types"
///   xsi:type="d3p1:Type">d3p2:Undefined</right>`, `<value xmlns:d5p1="…8.1/data/ui"
///   xsi:type="d5p1:Color">`.
/// ЭЛЕМЕНТ в ns без префикса в области видимости переопределяет ДЕФОЛТНЫЙ ns
/// (`<item xmlns="…dcs/core" xmlns:dcsset="…settings" xsi:type="dcsset:SettingsParameterValue">`
/// внутри `<appearance>`); QName-значение (`xsi:type`, текст `v8:Type`) — наоборот, требует
/// ПРЕФИКСА (пустой годится, только если дефолтный ns уже тот).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DcsSettingsSection {
    /// Ключ бэга `Order` — группа `order` под корнем `<Order>`.
    Order,
    /// Ключ бэга `Filter` — группа `filter` под корнем `<Filter>`.
    Filter,
    /// Ключ бэга `Appearance` — группа `conditionalAppearance` под корнем `<ConditionalAppearance>`.
    Appearance,
    /// Ключ бэга `Group` — ГРУППИРОВКИ структуры под корнем `<GroupItems>`: дети `groupItems`
    /// КОРНЕВОГО `StructureItemGroup` (witness ДоступныеАнкеты.АрхивАнкет), а не сам элемент.
    Group,
    /// Ключ бэга `DataParameters` — `dataParameters` под корнем `<DataParameterValues>`. В отличие
    /// от четырёх прочих секций ПРИСУТСТВУЕТ ВСЕГДА (пустая на 228 из 229 динсписков SSL — потому
    /// F-wave 11 и запекла её как «константу»; единственный носитель НЕ-пустых параметров данных,
    /// `СценарииОбменовДанными.НастройкаРасписанияОбменовДанными`, получил бы молча НЕВЕРНУЮ ячейку).
    DataParameters,
}

/// Канонический `userSettingID` группы отбора (платформенный дефолт).
const DEFAULT_FILTER_ID: &str = "dfcece9d-5077-440b-b6b3-45a5cb4538eb";
/// Канонический `userSettingID` группы порядка.
const DEFAULT_ORDER_ID: &str = "88619765-ccb3-46c6-ac52-38e9c992ebd4";
/// Канонический `userSettingID` группы условного оформления.
const DEFAULT_APPEARANCE_ID: &str = "b75fecce-942b-4aed-abc9-e6a02e460fb3";
/// Канонический `itemsUserSettingID` структуры.
const DEFAULT_ITEMS_ID: &str = "911b6018-f537-43e8-a417-da56b22f9aec";

impl DcsSettingsSection {
    /// ОПЦИОНАЛЬНЫЕ секции (`DataParameters` присутствует всегда и сюда не входит).
    pub const ALL: [DcsSettingsSection; 4] =
        [Self::Order, Self::Filter, Self::Appearance, Self::Group];

    /// Ключ ячейки в бэге динсписка.
    pub fn bag_key(self) -> &'static str {
        match self {
            Self::Order => "Order",
            Self::Filter => "Filter",
            Self::Appearance => "Appearance",
            Self::Group => "Group",
            Self::DataParameters => "DataParameters",
        }
    }

    /// Тип-guid ячейки `{"#",<guid>,…}` — КОНСТАНТА секции (сверено по всем витнессам SSL).
    pub fn type_guid(self) -> &'static str {
        match self {
            Self::Order => "11743ff3-2db3-4cfc-9404-90ed8209437f",
            Self::Filter => "f6841c6b-6c71-4c82-ae9e-d08b49db326c",
            Self::Appearance => "93de27ad-a2d8-4b10-a82b-483c9b0648fe",
            Self::Group => "e2e2f2e9-e309-4212-9c70-3ab32dd93b4d",
            Self::DataParameters => "6217eee1-6289-49d2-9315-d87888cd2d62",
        }
    }

    /// Local-name корня XML-документа секции.
    fn root_local(self) -> &'static str {
        match self {
            Self::Order => "Order",
            Self::Filter => "Filter",
            Self::Appearance => "ConditionalAppearance",
            Self::Group => "GroupItems",
            Self::DataParameters => "DataParameterValues",
        }
    }

    /// Отклоняется ли ЭТА часть настроек от платформенного дефолта (⟺ секция в бэге ЕСТЬ).
    /// `None` = сайдкара нет (`<ListSettings/>`) ⇒ отклоняются ВСЕ ЧЕТЫРЕ (пустые секции).
    /// `DataParameters` присутствует ВСЕГДА (см. вариант) — предикат для неё тождественно ИСТИНА.
    pub fn deviates(self, ls: Option<&DcsListSettings>) -> bool {
        if self == Self::DataParameters {
            return true;
        }
        let Some(ls) = ls else { return true };
        /// Группа == платформенная заглушка (нет элементов, `viewMode=Normal`, канон-id, БЕЗ
        /// группового представления)?
        fn is_default(g: &Option<DcsSettingsGroup>, id: &str) -> bool {
            g.as_ref().is_some_and(|g| {
                g.items.is_empty()
                    && g.view_mode.as_deref() == Some("Normal")
                    && g.user_setting_id.as_deref() == Some(id)
                    && g.user_setting_presentation.is_none()
            })
        }
        match self {
            Self::Order => !is_default(&ls.order, DEFAULT_ORDER_ID),
            Self::Filter => !is_default(&ls.filter, DEFAULT_FILTER_ID),
            Self::Appearance => !is_default(&ls.conditional_appearance, DEFAULT_APPEARANCE_ID),
            Self::Group => {
                !ls.structure_items.is_empty()
                    || ls.items_view_mode.as_deref() != Some("Normal")
                    || ls.items_user_setting_id.as_deref() != Some(DEFAULT_ITEMS_ID)
                    // itemsUserSettingPresentation (SC4) живёт в квартете Group (ячейка
                    // GroupSelectedSettingPresentation) ⇒ его наличие тоже порождает секцию.
                    || ls.items_user_setting_presentation.is_some()
            }
            Self::DataParameters => unreachable!("handled above"),
        }
    }

    /// ДЕТИ корня секции (Designer-префиксованные), из настроек списка.
    fn children(self, ls: Option<&DcsListSettings>) -> Vec<OutElement> {
        let Some(ls) = ls else { return Vec::new() };
        let group = |g: &Option<DcsSettingsGroup>| {
            g.as_ref()
                .map(designer_dcs_group_children)
                .unwrap_or_default()
        };
        match self {
            Self::Order => group(&ls.order),
            Self::Filter => group(&ls.filter),
            Self::Appearance => group(&ls.conditional_appearance),
            // `<GroupItems>` несёт ПЛОСКИЙ список `GroupItemField` всего дерева структуры в
            // pre-order-обходе: поля `groupItems` каждой `StructureItemGroup`, затем — рекурсивно
            // поля её ВЛОЖЕННЫХ под-структур (сверено с erp.cf ВидыЦен: СсылкаВидЦен, Валюта,
            // Выбран, Картинка, СпособЗаданияЦены, Статус). Designer/edt держат дерево вложенным.
            Self::Group => {
                let mut out = Vec::new();
                for it in &ls.structure_items {
                    flatten_structure_group_items(it, &mut out);
                }
                out
            }
            Self::DataParameters => ls
                .data_parameters
                .iter()
                .map(designer_dcs_settings_parameter_value)
                .collect(),
        }
    }
}

/// Уплощить дерево структуры динсписка в ПЛОСКИЙ список `GroupItemField` для cf-секции
/// `<GroupItems>` (pre-order: поля `groupItems` узла, затем рекурсивно — его вложенные
/// под-структуры). Для одноуровневой структуры (без вложений) даёт РОВНО прежний результат
/// (поля корневой группы); вложенные уровни ДОБАВЛЯЮТСЯ в обходе (byte-exact к erp.cf).
fn flatten_structure_group_items(it: &DcsItem, out: &mut Vec<OutElement>) {
    match it {
        DcsItem::StructureGroup {
            group_items,
            nested,
        } => {
            for g in group_items {
                out.push(designer_dcs_item(g));
            }
            for n in nested {
                flatten_structure_group_items(n, out);
            }
        }
        other => out.push(designer_dcs_item(other)),
    }
}

/// Ns-блок корня секции — РОВНО три объявления (77/77 витнессов идентичны).
const SECTION_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns", DCSSET_NS_URI),
    ("xmlns:xs", XS_NS_URI),
    ("xmlns:xsi", XSI_NS_URI),
];

/// Сериализовать СЕКЦИЮ настроек в байты XML-документа (то, что base64-ится в ячейку бэга).
/// `ls` = `None` ⇒ сайдкара нет ⇒ ПУСТОЙ корень секции.
///
/// §1.0: неизвестный Designer-префикс / неразрешимая ссылка стиля — громкая ошибка, не
/// молчаливый пропуск.
pub fn write_list_settings_section(
    section: DcsSettingsSection,
    ls: Option<&DcsListSettings>,
    style_item_uuid: StyleItemUuid<'_>,
) -> Result<Vec<u8>, FormError> {
    let mut root = OutElement::self_closing("", section.root_local());
    for (name, uri) in SECTION_ROOT_NS {
        root = root.attr(*name, *uri);
    }
    // Область видимости корня: дефолтный ns = settings, плюс xs/xsi.
    let scope: Vec<(String, &str)> = vec![
        (String::new(), DCSSET_NS_URI),
        ("xs".into(), XS_NS_URI),
        ("xsi".into(), XSI_NS_URI),
    ];
    for child in section.children(ls) {
        let out = reprefix(&child, 2, &scope, style_item_uuid)?;
        root.self_closing = false;
        root.push(out);
    }
    Ok(render(&section_envelope(), &root))
}
