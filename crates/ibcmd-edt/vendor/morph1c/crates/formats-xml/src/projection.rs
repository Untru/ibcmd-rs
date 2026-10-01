//! Драйвер проекции XML: карта локусов [`LocusMap`], источник/приёмник объекта
//! ([`XmlSource`]/[`XmlSink`]) и реализация [`morph1c_core::engine::Projection`]
//! ([`XmlProjection`]), драйвящая `core::engine::{read,write}` по [`crate::locus`].

use crate::codec::{
    claim_locus, decode_localized_keyval_edt, decode_use_purposes, decode_with_codec, edt_list_tag,
    encode_with_codec, Located,
};
use crate::{
    characteristics, choice_param_links, choice_parameters, common_attribute_content,
    configuration, exchange_plan_content, predefined, predefined_cct, predefined_coa, ref_list,
    std_attrs_generic, std_attrs_ir, std_tabular_sections, xdto_packages,
};
use crate::{ChildLocus, Codec, Element, FieldProjection, OutElement, XmlLocus};
use morph1c_core::engine::{Decoded, Projection};
use morph1c_core::ir::value::{PropertyValue, ValueKind};
use morph1c_core::ir::FieldId;
use morph1c_core::version::FormatVersion;

/// Карта проекции формата: для каждого `FieldId` его локус+кодек. Реализуется
/// коннектором формата (EDT/Designer) и передаётся в [`XmlProjection`].
///
/// Возвращает ВЛАДЕЕМЫЙ [`FieldProjection`] (он дёшев: пара мелких enum'ов + путь
/// `&'static [&'static str]`), чтобы реализация не держала статическую таблицу и не
/// заимствовала — карта мала (≈10 полей на вид) и строится на лету.
pub trait LocusMap {
    /// Локус+кодек поля, либо `None` если поле не проецируется этим форматом.
    fn lookup(&self, field: FieldId) -> Option<FieldProjection>;

    /// Локус ДОЧЕРНЕЙ КОЛЛЕКЦИИ по её формат-нейтральному коду
    /// (`ChildSlot.collection`), либо `None` если вид не несёт такой коллекции
    /// (дефолт для лист-видов — child-objects substrate, §3.4). Имена тегов
    /// контейнера/элемента/идентичности — здесь, не в спеке (§1.6).
    fn child_collection(&self, _collection: &str) -> Option<ChildLocus> {
        None
    }

    /// Привязки child-видов (спек+проекция каждого) в ЭТОМ формате — по одной на
    /// `ChildSlot` родителя. Дефолт `&[]` — лист-вид без детей. Рекурсивный слой
    /// (`children::read_children`/`write_children`) берёт отсюда чем читать ребёнка.
    fn child_bindings(&self) -> &'static [crate::children::ChildBinding] {
        &[]
    }

    /// ДОПОЛНИТЕЛЬНЫЕ объявления ns на корне дескриптора (`(имя_атрибута, URI)`), сверх
    /// базового envelope формата — ПЕР-ВИД envelope-константа. Напр. EDT `Enum` несёт
    /// `xmlns:xsi`+`xmlns:core` (их требует const-блок `standardAttributes` с
    /// `xsi:type="core:UndefinedValue"`). Дефолт `&[]` — вид без доп. ns (CommonModule).
    /// Порядок = порядку эталона (доп. ns идут ДО `xmlns:mdclass`). Коннектор claim'ит
    /// их на чтении (сверка URI) и воспроизводит на записи byte-exact.
    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// Доп. ns-объявления корня, физически идущие ПОСЛЕ базового `xmlns:mdclass` (EDT).
    /// Дефолт `&[]` — все доп. ns идут ДО базового (порядок всех прежних видов: xsi/core →
    /// mdclass). Единственный witnessed-носитель хвостовой позиции — `xmlns:mdclassExtension`
    /// корня РАСШИРЕНИЯ (s13: `xsi, mdclass, mdclassExtension`). Семантика та же, что у
    /// [`Self::root_extra_namespaces`]: claim+сверка URI на чтении, declare-iff-used на записи.
    fn root_extra_namespaces_trailing(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// Поля, чьи property-узлы Designer физически размещает ПЕРЕД каркасным листом `<Name>`
    /// в `<Properties>` (witnessed: `ObjectBelonging` корня РАСШИРЕНИЯ — единственный
    /// известный случай). Дефолт `&[]` — все свойства после `<Name>` (все прежние виды
    /// байт-идентичны). Read позиционно-независим — это лишь WRITE-хук (как
    /// [`Self::trailing_fields`]).
    fn properties_head_fields(&self) -> &'static [FieldId] {
        &[]
    }

    /// УСЛОВНАЯ ли эмиссия [`Self::root_extra_namespaces`] для КОНКРЕТНОГО объекта.
    /// Дефолт `true` — доп. ns присутствуют ВСЕГДА (все виды до DataProcessor: их
    /// const-блоки `standardAttributes`/`producedTypes` всегда требуют `xsi`/`core`).
    /// `DataProcessor` переопределяет: `xmlns:xsi`+`xmlns:core` объявляются на корне
    /// ТОЛЬКО когда объект несёт ≥1 дочерний `Attribute`/`TabularSection` (их `minValue`/
    /// `maxValue`/`fillValue` используют `xsi:type="core:…"`); без них корень их НЕ несёт
    /// (сверено по корпусу: 33/78 объявляют ns ⟺ 33/78 имеют такие дети). На записи доп. ns
    /// эмитятся, лишь если этот предикат вернул `true`; на чтении (для видов, где он МОЖЕТ
    /// вернуть `false` — см. [`Self::root_extra_namespaces_optional`]) отсутствие ns не есть
    /// ошибка. Round-trip byte-exact держится корпус-инвариантом.
    fn root_extra_namespaces_present(&self, _obj: &morph1c_core::ir::MetadataObject) -> bool {
        true
    }

    /// Может ли [`Self::root_extra_namespaces_present`] вернуть `false` для какого-то
    /// объекта (т.е. доп. ns УСЛОВНЫ). Дефолт `false` — ns обязательны (их отсутствие на
    /// чтении → envelope-ошибка). `DataProcessor` → `true` (ns правомерно отсутствуют у
    /// объектов без Attribute/TabularSection).
    fn root_extra_namespaces_optional(&self) -> bool {
        false
    }

    /// Физический порядок ЭМИССИИ полей в дескрипторе ЭТОГО формата, если он расходится
    /// с каноническим порядком спека (§1.6/§3.2). `None` (дефолт) ⇒ порядок спека —
    /// верно для большинства видов/форматов (корень InformationRegister, все leaf-виды
    /// F2b). `Some(&[FieldId…])` ⇒ узлы переупорядочиваются под этот порядок тегов
    /// формата ([`XmlSink::ordered`]); напр. EDT-дети `Resource`/`Dimension`/`Attribute`
    /// эмитят свойства в ином порядке, чем Designer-DENSE.
    ///
    /// Это НЕ ослабляет §1.6: канонический IR (порядок свойств в bag) задаёт спек ОДИН
    /// раз; здесь — лишь физическое РАЗМЕЩЕНИЕ тегов на записи (как `XmlLocus` — лишь имя
    /// тега). Read обоих форматов даёт тот же bag в спек-порядке независимо от физики.
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        None
    }

    /// Поля recursion-узла, физически эмитируемые ПОСЛЕ inline-детей (а не до), в ЭТОМ
    /// формате. Дефолт `&[]` — все свойства идут до детей (верно для всех видов, кроме
    /// EDT `Catalog.TabularSection`, где `<use>` стоит ПОСЛЕ `<attributes>`). Read
    /// позиционно-независим (свойства читаются по локусу), поэтому это лишь WRITE-хук.
    /// Эти поля исключаются из обычной (до-детей) эмиссии и дописываются после.
    fn trailing_fields(&self) -> &'static [FieldId] {
        &[]
    }

    /// Эмитить ли ПУСТУЮ обёртку дочерней коллекции (Designer `<ChildObjects/>`), когда
    /// детей нет. Дефолт `false` — пустую обёртку опускаем (Enum/IR всегда несут ≥1
    /// ребёнка, поэтому не задеты). `true` для `Catalog`: ВСЕ 74 Designer-объекта несут
    /// `<ChildObjects>` (1 — пустой `<ChildObjects/>` при нуле детей — сверено). Только
    /// Designer-релевантно (EDT детей не оборачивает).
    fn emit_empty_child_container(&self) -> bool {
        false
    }

    /// ДОТИРОВАННЫЙ bare-ref: текст ссылки — не голое имя, а полный путь
    /// `<ParentKind>.<ParentName>.<Seg>.<Имя>` (`Seg` — последний дот-сегмент child-вида).
    /// Witness: EDT `ExternalDataSource` — `<tables>ExternalDataSource.BaseBU.Table.<Имя>
    /// </tables>` (ERP BaseBU). Чтение СРЕЗАЕТ префикс (канон-имя ребёнка — ГОЛОЕ, как у
    /// Designer bare-ref той же коллекции, §1.6; несовпавший префикс → типизированная
    /// ошибка §1.0), запись — детерминированно восстанавливает (R byte-exact).
    /// Default `false` — обычный bare-ref (голое имя).
    fn bare_ref_dotted(&self, _collection: &str) -> bool {
        false
    }

    /// ПЕР-ФОРМАТНЫЙ дефолт поля (форм-атрибуты L1f, §1.6) — пробрасывается в
    /// [`morph1c_core::engine::Projection::field_default`]. Дефолт `None` ⇒ канонический
    /// `fs.default` (поведение ВСЕХ существующих видов байт-идентично). Форм-проекции
    /// (EDT/Designer) переопределяют для атрибутов с ПРОТИВОПОЛОЖНЫМИ дефолтами форматов.
    fn field_default(&self, _field: FieldId) -> Option<PropertyValue> {
        None
    }
}

/// Источник одного объекта для read-движка: ВЛАДЕЕТ корнем дескриптора. Учёт
/// тотальности — через внутреннюю изменяемость [`Element`] (`claimed: Cell`), поэтому
/// `decode_field(&self, …)` помечает узлы без `&mut`. Владение (а не заимствование)
/// снимает лайфтайм с `Projection::Source` (у трейта он без параметра-лайфтайма).
///
/// Коннектор формата ОБЯЗАН до запуска движка claim'нуть каркасные узлы (атрибуты
/// `xmlns:*`/`uuid` корня, элемент `name`), иначе они попадут в `leftover` (§1.0).
pub struct XmlSource {
    /// Корневой элемент объекта (владение).
    pub root: Element,
    /// Канонические id полей спека (в порядке спека) — нужны `leftover`-проходу,
    /// чтобы claim'нуть всё локус-достижимое ДО `decode_field` (контракт движка).
    pub fields: Vec<FieldId>,
}

/// порядке вызовов `encode_field` (= канонический порядок спека).
///
/// ПОРЯДОК ЭМИССИИ ПЕР-ФОРМАТ (§1.6/§3.2): движок эмитит поля в каноническом порядке
/// СПЕКА, но физический порядок тегов в дескрипторе МОЖЕТ расходиться между форматами
/// (для InformationRegister дочерние `Resource`/`Dimension`/`Attribute` идут в РАЗНОМ
/// порядке у EDT и Designer — сверено по корпусу). Поэтому каждый эмитированный узел
/// помечается своим [`FieldId`] (`order_tags`), а коннектор переупорядочивает их под
/// физический порядок формата через [`XmlSink::ordered`], если проекция его задаёт
/// ([`LocusMap::field_emit_order`]). Узлы ОДНОГО поля (напр. 4 блока вариативного
/// `standardAttributes`) держатся ГРУППОЙ и не разрываются.
#[derive(Debug, Default)]
pub struct XmlSink {
    /// Дочерние элементы дескриптора в порядке эмиссии (= спек-порядок).
    pub children: Vec<OutElement>,
    /// Канонический [`FieldId`] поля, эмитировавшего каждый узел `children` (1:1, тот же
    /// порядок). Заполняется [`XmlProjection::encode_field`]; используется
    /// [`XmlSink::ordered`] для пер-форматного переупорядочивания.
    pub order_tags: Vec<FieldId>,
}

impl XmlSink {
    /// Вернуть узлы в физическом порядке формата (`order` — список [`FieldId`] в
    /// порядке тегов формата). Узлы поля, отсутствующего в `order`, сохраняют исходное
    /// относительное положение в конце (стабильно). Если `order` пуст/`None` — порядок
    /// спека (как накоплено).
    ///
    /// Группы узлов одного поля НЕ разрываются: сортировка стабильна по индексу поля в
    /// `order` (узлы с одинаковым тегом-полем остаются в исходном порядке).
    pub fn ordered(self, order: Option<&[FieldId]>) -> Vec<OutElement> {
        let order = match order {
            Some(o) if !o.is_empty() => o,
            _ => return self.children,
        };
        let rank =
            |id: FieldId| -> usize { order.iter().position(|x| *x == id).unwrap_or(order.len()) };
        let mut indexed: Vec<(usize, usize, OutElement)> = self
            .children
            .into_iter()
            .zip(self.order_tags)
            .enumerate()
            .map(|(i, (el, id))| (rank(id), i, el))
            .collect();
        // Стабильно по (rank поля, исходный индекс) — группы поля цельны, неизвестные
        // поля (rank==len) уходят в конец в исходном порядке.
        indexed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        indexed.into_iter().map(|(_, _, el)| el).collect()
    }
}

/// Реализация [`Projection`] поверх карты локусов и envelope формата.
///
/// Драйвит `core::engine::{read,write}`: движок walk'ает канонический спек и для
/// каждого поля зовёт сюда; мы локализуем ячейку по [`XmlLocus`] и кодируем/декодим
/// по [`Codec`]. Канонику (id/kind/порядок/дефолты) решает движок+спек, не мы (§1.6).
pub struct XmlProjection<'m, M: LocusMap + ?Sized> {
    map: &'m M,
    version: FormatVersion,
    /// Эмитить ли дефолтные поля (EDT: нет, re-sparsify; Designer: да).
    emit_defaults: bool,
}

impl<'m, M: LocusMap + ?Sized> XmlProjection<'m, M> {
    /// Создать проекцию: карта + версия формата + политика дефолтов.
    pub fn new(map: &'m M, version: FormatVersion, emit_defaults: bool) -> Self {
        XmlProjection {
            map,
            version,
            emit_defaults,
        }
    }

    /// Локализовать read-side элемент по локусу. Терминальный элемент claim'ится;
    /// его ns-префикс СВЕРЯЕТСЯ с `locus.ns` (B2: префикс — часть идентичности тега,
    /// иначе `<mdclass:server>` прочитался бы как `server` и потерял префикс).
    fn locate<'a>(&self, root: &'a Element, locus: &XmlLocus) -> Located<'a> {
        match locus {
            XmlLocus::RootAttr { name } => match root.attr(name) {
                Some(a) => {
                    a.claimed.set(true);
                    Located::Attr(&a.value)
                }
                None => Located::Absent,
            },
            XmlLocus::PropElement { path, ns } => {
                let mut cur = root;
                for (i, seg) in path.iter().enumerate() {
                    match cur.child(seg) {
                        Some(child) => {
                            if i + 1 == path.len() {
                                // Терминальный элемент: сверяем префикс с ожидаемым.
                                if child.prefix != *ns {
                                    return Located::PrefixMismatch {
                                        local: child.local.clone(),
                                        want: ns,
                                        got: child.prefix.clone(),
                                    };
                                }
                                child.claim();
                                return Located::Element(child);
                            }
                            cur = child;
                        }
                        None => return Located::Absent,
                    }
                }
                // Пустой путь не используется.
                Located::Absent
            }
        }
    }
}

impl<M: LocusMap + ?Sized> Projection for XmlProjection<'_, M> {
    type Source = XmlSource;
    type Sink = XmlSink;

    fn format_version(&self) -> FormatVersion {
        self.version
    }

    fn decode_field(&self, source: &Self::Source, field: FieldId, expected: ValueKind) -> Decoded {
        let fp = match self.map.lookup(field) {
            Some(fp) => fp,
            // Поле спека, которое формат не проецирует, — для F2b все 10 проецируются.
            None => return Decoded::Absent,
        };
        // Платформенные блоки standardAttributes навигируют от КОРНЯ сами (EDT — много
        // одноимённых блоков, обычный single-element `locate` их не покрыл бы).
        if let Codec::IrStandardAttributes(dialect, decl) = fp.codec {
            return std_attrs_ir::decode(dialect, decl, &source.root, self.version);
        }
        if let Codec::UsePurposesConst = fp.codec {
            return decode_use_purposes(&source.root);
        }
        if let Codec::StdAttrs {
            dialect,
            decl,
            variant,
        } = fp.codec
        {
            return std_attrs_generic::decode(dialect, decl, variant, &source.root, self.version);
        }
        // standardTabularSections — тоже от КОРНЯ (EDT — несколько inline-блоков).
        if let Codec::StdTabularSections { dialect, decl } = fp.codec {
            return std_tabular_sections::decode(dialect, decl, &source.root, self.version);
        }
        if let Codec::ExchangePlanContent = fp.codec {
            return exchange_plan_content::decode_edt(&source.root);
        }
        // CommonAttribute content (EDT) — multi-sibling `<content>` от КОРНЯ. Designer —
        // single-container (через locate, ниже).
        if let Codec::CommonAttributeContent(
            crate::common_attribute_content::CommonAttributeContentDialect::Edt,
        ) = fp.codec
        {
            return common_attribute_content::decode_edt(&source.root);
        }
        // EDT-локализация — MULTI-SIBLING от КОРНЯ (§1.0): EDT эмитит по ОДНОМУ
        // `<tag>` на язык (двуязычный ERP: `<synonym>ru</…><synonym>en</…>`),
        // single-element `locate` увидел бы лишь первый → второй язык остался бы
        // неклеймнутым (UnconsumedInput). Читаем ВСЕ сиблинги `<tag>` в порядке
        // источника (как `RefList`/`Characteristics`-Edt). Designer — single-container
        // `LocalizedV8` (через locate; уже читает все `<v8:item>`).
        if let Codec::LocalizedKeyVal = fp.codec {
            return decode_localized_keyval_edt(&source.root, edt_list_tag(&fp.locus));
        }
        // RefList(Edt) и Characteristics(Edt) — multi-sibling от КОРНЯ (как
        // choiceParameterLinks-EDT). Designer-варианты — single-container (через locate).
        if let Codec::RefList(crate::ref_list::RefListDialect::Edt) = fp.codec {
            return ref_list::decode_edt(&source.root, edt_list_tag(&fp.locus));
        }
        if let Codec::Characteristics(crate::characteristics::CharacteristicsDialect::Edt) =
            fp.codec
        {
            return characteristics::decode_edt(&source.root);
        }
        if let Codec::ChoiceParameters(crate::choice_parameters::ChoiceParametersDialect::Edt) =
            fp.codec
        {
            return choice_parameters::decode_edt(&source.root);
        }
        // ChoiceParameterLinks(Edt) — multi-sibling от КОРНЯ (несколько связей = несколько
        // сиблингов; AccumulationRegister-измерения несут >1). Designer — single-container
        // (через locate, ниже). Single-host остаётся ВНУТРИ std-attrs (OptCpl).
        if let Codec::ChoiceParameterLinks(crate::choice_param_links::LinksDialect::Edt) = fp.codec
        {
            return choice_param_links::decode_edt_from_root(&source.root);
        }
        // xdtoPackages(Edt) — multi-sibling `<xdtoPackages xsi:type>` от КОРНЯ (0..N;
        // WebService несёт 0 или 1). Designer — single-container (через locate, ниже).
        if let Codec::XdtoPackages(crate::xdto_packages::XdtoPackagesDialect::Edt) = fp.codec {
            return xdto_packages::decode_edt(&source.root, edt_list_tag(&fp.locus));
        }
        if let Codec::PredefinedData = fp.codec {
            return predefined::decode_edt(&source.root);
        }
        if let Codec::PredefinedDataCct = fp.codec {
            return predefined_cct::decode_edt(&source.root);
        }
        if let Codec::PredefinedDataCoa = fp.codec {
            return predefined_coa::decode_edt(&source.root);
        }
        // Configuration from-root коды (multi-node / фикс-обёртка под корнем).
        if let Codec::ContainedObjects(dialect) = fp.codec {
            return configuration::decode_contained_objects(dialect, &source.root);
        }
        if let Codec::ConfigChildObjects(dialect) = fp.codec {
            return configuration::decode_child_objects(dialect, &source.root);
        }
        if let Codec::LanguagesEntity = fp.codec {
            return configuration::decode_languages_edt(&source.root);
        }
        let located = self.locate(&source.root, &fp.locus);
        decode_with_codec(&fp.codec, located, expected, self.version)
    }

    fn encode_field(
        &self,
        sink: &mut Self::Sink,
        field: FieldId,
        value: &PropertyValue,
        is_default: bool,
    ) -> Result<(), String> {
        // Разреженный формат (EDT) дефолты не эмитит (re-sparsify); плотный — эмитит.
        if is_default && !self.emit_defaults {
            return Ok(());
        }
        let fp = match self.map.lookup(field) {
            Some(fp) => fp,
            None => return Ok(()),
        };
        let before = sink.children.len();
        encode_with_codec(&fp.locus, &fp.codec, value, sink, self.version)?;
        // Пометить КАЖДЫЙ только что эмитированный узел его полем (для пер-форматного
        // переупорядочивания, см. `XmlSink::ordered`). Большинство кодеков дают 1 узел;
        // вариативный standardAttributes — несколько (все одного поля → группа цельна).
        for _ in before..sink.children.len() {
            sink.order_tags.push(field);
        }
        Ok(())
    }

    fn field_default(&self, field: FieldId) -> Option<PropertyValue> {
        // Делегируем карте проекции: форм-проекции задают пер-форматные дефолты (§1.6);
        // metadata-карты возвращают `None` ⇒ канонический `fs.default` (без изменений).
        self.map.field_default(field)
    }

    fn leftover(&self, source: &Self::Source) -> usize {
        // §1.0: ни один узел поддерева корня не должен остаться невостребованным.
        //
        // Контракт движка: `leftover` вызывается ДО `decode_field`. Поэтому здесь же
        // выполняем claim-проход по всем полям карты — помечаем КАЖДУЮ ячейку, которую
        // проекция СПОСОБНА разобрать (её локус достижим). То, что осталось не
        // помечено (после каркасных claim'ов коннектора), — реально несконсуменное.
        for &field in &source.fields {
            if let Some(fp) = self.map.lookup(field) {
                claim_locus(&source.root, &fp.locus, &fp.codec, self.version);
            }
        }
        source.root.unclaimed_count()
    }

    fn leftover_names(&self, source: &Self::Source) -> Vec<String> {
        // Тот же claim-проход, что в `leftover`, затем перечисляем неразобранные элементы.
        for &field in &source.fields {
            if let Some(fp) = self.map.lookup(field) {
                claim_locus(&source.root, &fp.locus, &fp.codec, self.version);
            }
        }
        source.root.unclaimed_names(16)
    }
}

/// Property-claim-проход по `root` для child-objects substrate (§1.0, фикс nit-1):
/// помечает РОВНО те property-узлы, которые покрывает проекция `map` для полей `fields`
/// — тот же claim, что движок делает в [`XmlProjection::leftover`], но направленный на
/// КОНКРЕТНЫЙ узел (оригинал property-региона ребёнка), а не на источник движка.
///
/// Используется [`children::read_one`]: движок проверяет property-тотальность на КЛОНЕ
/// property-региона (без вложенных child-контейнеров), а этот хелпер переносит claim на
/// ОРИГИНАЛ, чтобы родительский `unclaimed_count` увидел свойства востребованными. ns/
/// версия/политика дефолтов берутся как у движка (claim не зависит от значения).
pub fn claim_props<M: LocusMap + ?Sized>(
    map: &M,
    version: FormatVersion,
    _emit_defaults: bool,
    root: &Element,
    fields: &[FieldId],
) {
    for &field in fields {
        if let Some(fp) = map.lookup(field) {
            claim_locus(root, &fp.locus, &fp.codec, version);
        }
    }
}
