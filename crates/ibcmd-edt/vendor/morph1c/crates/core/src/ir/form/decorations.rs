use super::*;

/// Композит-шрифт контрола: ССЫЛКА (`core:FontRef`, [`Self::font_ref`]`=Some`) либо
/// АБСОЛЮТ (`core:FontDef`, `font_ref=None`). Канон-`font_ref` — EDT-форма (`Style.X`/
/// `System.X`); `kind` НЕ хранится (Ref: выводится из префикса ссылки `Style.`→`StyleItem`,
/// `System.`→`WindowsFont`; Def: константа `Absolute`). `height` хранится в EDT-форме
/// (`11.0`); Designer эмитит без `.0` (`11`). Флаги — пер-компонентные переопределения.
///
/// # FontDef — противоположные плотности эмиссии (§1.6)
/// EDT эмитит SPARSE (только не-дефолт: witness `<font xsi:type="core:FontDef">
/// <height>11.0</height></font>`); Designer — DENSE (`<Font faceName="" height="11"
/// bold="false" … kind="Absolute" scale="100"/>`). Канон хранит sparse-форму: Designer-ридер
/// сводит дефолты (`faceName=""`→None, флаг `false`→None, `scale=100`→None), Designer-писатель
/// реконструирует dense.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontRef {
    /// АВТО-шрифт: базовая гарнитура/высота НАСЛЕДУЕТСЯ (авто), а пер-компонентные
    /// переопределения (`bold`/`italic`/…) применяются ПОВЕРХ. `true` ⇒ EDT
    /// `<font xsi:type="core:AutoFont">…` ⟺ Designer `<Font … kind="AutoFont"/>`. Отличает
    /// AutoFont от абсолюта (`core:FontDef`/`kind="Absolute"`) — у ОБОИХ [`Self::font_ref`]`=None`,
    /// поэтому нужен явный дискриминатор (§1.0). Форм-корпус витнессит AutoFont ТОЛЬКО с одним
    /// флагом-переопределением (`bold`/`italic`), пустой AutoFont в форм-контексте не встречается
    /// (пустой = дефолт = отсутствие узла; пустые AutoFont'ы живут лишь в chart/flowchart/mxl,
    /// у которых собственный font-кодек). cf: mode 3 (`{8,3,…}`) — та же ветвь, что у отсутствующего
    /// шрифта, плюс override-ячейки (НЕ витнесснуто на cf-стороне — до-майнится абляцией).
    #[serde(default)]
    pub auto: bool,
    /// Ссылка на шрифт в EDT-форме: `Style.<Имя>` / `System.<Имя>`. `None` ⇒ АБСОЛЮТНЫЙ
    /// шрифт (`core:FontDef` ⟺ Designer `kind="Absolute"` без `@ref`) ЛИБО AutoFont
    /// ([`Self::auto`]`=true`).
    pub font_ref: Option<String>,
    /// Имя гарнитуры-переопределения (`Times New Roman`), если задано.
    #[serde(default)]
    pub face_name: Option<String>,
    /// Высота в EDT-форме (`11.0`), если задана. Designer эмитит без `.0`.
    #[serde(default)]
    pub height: Option<String>,
    /// Жирный (переопределение), если задан.
    #[serde(default)]
    pub bold: Option<bool>,
    /// Курсив (переопределение), если задан.
    #[serde(default)]
    pub italic: Option<bool>,
    /// Подчёркнутый (переопределение), если задан.
    #[serde(default)]
    pub underline: Option<bool>,
    /// Зачёркнутый (переопределение), если задан.
    #[serde(default)]
    pub strikeout: Option<bool>,
    /// Масштаб (`100`), если задан. Designer эмитит его ПОСЛЕДНИМ атрибутом (после `kind`).
    #[serde(default)]
    pub scale: Option<String>,
}

/// Decorator-под-контрол (расширенная подсказка / контекстное меню): имя + id + ТЕЛО.
///
/// # Тело — ДАННЫЕ, и его СОДЕРЖАНИЕ X-сравнимо (ОБА формата его несут)
/// Расширенная подсказка — вложенный `LabelDecoration`-стаб; его тело ВАРЬИРУЕТСЯ (несёт
/// `title`/`maxWidth`/`autoMaxWidth`/`autoMaxHeight`/`horizontalStretch`/`formatted`/
/// события). EDT держит его инлайн (`<extendedTooltip>…`); Designer — ТОЖЕ держит содержимое
/// инлайн (`<ExtendedTooltip name id><Title>…</Title>…`), либо bare-ref `<X name id/>` для
/// пустого тела. Поэтому тело ЧИТАЕТСЯ КАК ДАННЫЕ ([`Self::body`]) ОБОИМИ форматами и
/// СРАВНИВАЕТСЯ X (после реконсиляции пер-форматных дефолтов autoMax*/autoFill — оба ридера
/// дают РАВНЫЙ bag). EDT дополнительно несёт авто-КАРКАС (`type=Label`/extInfo-обёртка/
/// `horizontalAlign`) — форматно-асимметричную денормализацию без Designer-аналога; ЕЁ (и
/// только её) нормализатор X зануляет (`normalize_form_for_x`). Контекстное меню несёт
/// `autoFill` (пер-форматный дефолт) плюс иногда вложенное дерево кнопок (пока не модель.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecoratorRef {
    /// Имя под-контрола (`<имя>РасширеннаяПодсказка`/`<имя>КонтекстноеМеню`).
    pub name: String,
    /// `id` под-контрола.
    pub id: i64,
    /// Тело под-стаба как ДАННЫЕ: для расширенной подсказки — `Tooltip(LabelDecoration-
    /// свойства)`; для контекстного меню — `ContextMenu(autoFill)`. ОБА формата
    /// реконструируют свою кодировку ИЗ ЭТОГО; X сравнивает СОДЕРЖАНИЕ (кроме EDT-каркаса).
    pub body: DecoratorBody,
}

/// Тело decorator-под-стаба, читаемое как ДАННЫЕ для byte-exact R И для X.
///
/// ОБА формата несут содержимое тела (EDT — инлайн в `<extendedTooltip>`; Designer — инлайн
/// в `<ExtendedTooltip name id>…` или bare-ref при пустом теле). Содержимое X-сравнимо; лишь
/// EDT-каркас (extInfo-обёртка) форматно-асимметричен.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecoratorBody {
    /// Расширенная подсказка (`<extendedTooltip>`): вложенная надпись-`LabelDecoration`.
    /// Несёт её свойства/extInfo/события (тот же property-bag, что у обычного контрола),
    /// плюс флаг `formatted` (HTML-заголовок). `name`/`id`/`type=Label`/extInfo-обёртка
    /// реконструируются каркасом из (name, id) родительского [`DecoratorRef`].
    Tooltip(TooltipBody),
    /// Контекстное меню (`<contextMenu>`): авто-меню `<autoFill>` (как правило `true`).
    ContextMenu(ContextMenuBody),
}

/// Тело расширенной подсказки (вложенный `LabelDecoration`-стаб) — ДАННЫЕ для R И для X.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TooltipBody {
    /// Explicit EDT Auto spelling; current alignment remains semantic data.
    #[serde(skip)]
    pub edt_horizontal_align_auto_explicit: bool,
    /// Общие свойства вложенной надписи (`title`/`maxWidth`/`autoMaxWidth`/`autoMaxHeight`/
    /// `horizontalStretch`) по каноническому [`FieldId`], в КАНОНИЧЕСКОМ порядке (общий обоим
    /// форматам). Пер-форматные дефолты (`autoMax*`) реконсилированы на read ⇒ X-сравнимо.
    /// Дефолты НЕ хранятся (sparse-против-каноники).
    #[serde(serialize_with = "crate::ir::serialize_semantic_properties")]
    pub properties: Vec<(FieldId, PropertyValue)>,
    /// HTML-заголовок: EDT `<formatted>true</formatted>` ⟺ Designer `<Title formatted="true">`.
    /// X-сравним (оба формата несут).
    pub formatted: bool,
    /// Важность отображения расширенной подсказки: EDT `<displayImportance>High</displayImportance>`
    /// (ЧИЛД после `id`) ⟺ Designer `DisplayImportance="High"` (АТРИБУТ элемента `<ExtendedTooltip>`).
    /// ОБА формата несут ⇒ X-сравним. Witness DataProcessor.УточнениеУчетаНДФЛ…Форма кнопка
    /// «СоздатьДокументы» (`High`). cf-ячейка расширенной подсказки для неё НЕ витнесснута ⇒
    /// типизированный отказ cf-write (§1.0). `None` ⇒ тег/атрибут отсутствует.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_importance: Option<String>,
    /// extInfo-свойства вложенной надписи (`horizontalAlign`) — EDT-КАРКАС (Designer в теле
    /// подсказки его не несёт). Форматно-асимметричен ⇒ нормализуется ДО X.
    pub ext_info: Vec<(FieldId, PropertyValue)>,
    /// События вложенной надписи (`URLProcessing` → обработчик), в исходном порядке. X-сравнимы.
    pub events: Vec<FormEvent>,
    /// Шрифт вложенной надписи (witness ERP; cf-ячейка снята абляцией — {12}-суб-запись [18]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<FontRef>,
}

/// Тело контекстного меню (`<contextMenu>`) — ДАННЫЕ для R и X.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ContextMenuBody {
    /// Значение `<autoFill>` (пер-форматный дефолт `true`): EDT эмитит `<autoFill>true>` у
    /// авто-меню и ОПУСКАЕТ его у не-авто (в корпусе `<autoFill>false>` 0/0), Designer
    /// опускает `true` (свой дефолт) и эмитит `<Autofill>false>` у не-авто. Оба ридера
    /// реконсилируют ОТСУТСТВИЕ к `Some(false)` / `Some(true)` ⇒ значение X-сравнимо строго.
    /// `None` — лишь serde-дефолт (ридеры его не порождают).
    pub auto_fill: Option<bool>,
    /// СОБСТВЕННЫЕ пункты контекстного меню — вложенное дерево контролов (как правило
    /// `Button`/`ButtonGroup`/`Popup`). EDT — `<items>`-дети под `<contextMenu>` (ДО
    /// `<autoFill>`); Designer — `<ChildItems>` под `<ContextMenu>`. Каждый пункт —
    /// полноценный [`FormItem`] (рекурсивно). Пусто ⇒ авто-меню без явных пунктов
    /// (пилот/большинство). X-сравнимы (оба формата несут один список).
    #[serde(default)]
    pub items: Vec<FormItem>,
}

impl DecoratorRef {
    /// Построить decorator-ref с данным телом.
    pub fn new(name: impl Into<String>, id: i64, body: DecoratorBody) -> Self {
        DecoratorRef {
            name: name.into(),
            id,
            body,
        }
    }
}
