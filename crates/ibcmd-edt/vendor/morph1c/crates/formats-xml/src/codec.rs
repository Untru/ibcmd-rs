//! Интерпретатор кодеков XML: локализация ячейки ([`Located`]) и декод/энкод/claim
//! значения по [`crate::locus::Codec`] (`decode_with_codec`/`encode_with_codec`/
//! `claim_locus`) плюс связанные хелперы (localized/help/usePurposes/…).

use crate::compat_mode::{compat_canonical_to_designer, compat_designer_to_canonical};
use crate::{
    characteristics, choice_param_links, choice_parameters, common_attribute_content,
    configuration, exchange_plan_content, link_by_type, picture, predefined, predefined_cct,
    predefined_coa, ref_list, shortcut, std_attrs_generic, std_attrs_ir, std_tabular_sections,
    style_value_codec, transparent_pixel, type_codec, value_codec, xdto_packages, xdto_type_ref,
};
use crate::{Codec, Element, OutElement, XmlLocus, XmlSink};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{Lang, PropertyValue, Token, ValueKind};
use morph1c_core::version::FormatVersion;

/// Результат локализации ячейки на чтении.
pub(crate) enum Located<'a> {
    /// Элемент найден (его непосредственный текст/дети доступны).
    Element(&'a Element),
    /// Найдено как значение атрибута.
    Attr(&'a str),
    /// Элемент найден, но его ns-префикс не совпал с ожидаемым (B2) → ошибка кодека.
    PrefixMismatch {
        /// Local-name найденного элемента.
        local: String,
        /// Ожидаемый префикс (`""` = без префикса).
        want: &'static str,
        /// Фактический префикс.
        got: String,
    },
    /// Ячейки нет.
    Absent,
}

/// Пометить (claim) все узлы, которые покрывает локус поля. Идемпотентно. Для
/// локализованных контейнеров claim'ит и контейнер, и пары key/value (item/lang/
/// content) — ровно то, что разберёт соответствующий кодек.
pub(crate) fn claim_locus(root: &Element, locus: &XmlLocus, codec: &Codec, version: FormatVersion) {
    // Платформенные блоки standardAttributes клеймятся прямо от корня (структура
    // фиксирована диалектом, локус-путь к ним не применяется).
    if let Codec::IrStandardAttributes(dialect, decl) = codec {
        std_attrs_ir::claim(*dialect, decl, root, version);
        return;
    }
    if let Codec::UsePurposesConst = codec {
        claim_use_purposes(root);
        return;
    }
    if let Codec::StdAttrs {
        dialect,
        decl,
        variant,
    } = codec
    {
        std_attrs_generic::claim(*dialect, decl, *variant, root, version);
        return;
    }
    if let Codec::StdTabularSections { dialect, decl } = codec {
        std_tabular_sections::claim(*dialect, decl, root, version);
        return;
    }
    if let Codec::ExchangePlanContent = codec {
        exchange_plan_content::claim_edt(root);
        return;
    }
    if let Codec::CommonAttributeContent(
        crate::common_attribute_content::CommonAttributeContentDialect::Edt,
    ) = codec
    {
        common_attribute_content::claim_edt(root);
        return;
    }
    if let Codec::LocalizedKeyVal = codec {
        // EDT-локализация — multi-sibling от КОРНЯ: claim РОВНО столько же, сколько
        // читает `decode_localized_keyval_edt` (все `<tag>`-сиблинги + их key/value).
        claim_localized_keyval_edt(root, edt_list_tag(locus));
        return;
    }
    if let Codec::RefList(crate::ref_list::RefListDialect::Edt) = codec {
        ref_list::claim_edt(root, edt_list_tag(locus));
        return;
    }
    if let Codec::Characteristics(crate::characteristics::CharacteristicsDialect::Edt) = codec {
        characteristics::claim_edt(root);
        return;
    }
    if let Codec::ChoiceParameters(crate::choice_parameters::ChoiceParametersDialect::Edt) = codec {
        choice_parameters::claim_edt(root);
        return;
    }
    if let Codec::ChoiceParameterLinks(crate::choice_param_links::LinksDialect::Edt) = codec {
        choice_param_links::claim_edt_from_root(root);
        return;
    }
    if let Codec::XdtoPackages(crate::xdto_packages::XdtoPackagesDialect::Edt) = codec {
        xdto_packages::claim_edt(root, edt_list_tag(locus));
        return;
    }
    if let Codec::PredefinedDataCct = codec {
        predefined_cct::claim_edt(root);
        return;
    }
    if let Codec::PredefinedDataCoa = codec {
        predefined_coa::claim_edt(root);
        return;
    }
    if let Codec::PredefinedData = codec {
        predefined::claim_edt(root);
        return;
    }
    if let Codec::ContainedObjects(dialect) = codec {
        configuration::claim_contained_objects(*dialect, root);
        return;
    }
    if let Codec::ConfigChildObjects(dialect) = codec {
        configuration::claim_child_objects(*dialect, root);
        return;
    }
    if let Codec::LanguagesEntity = codec {
        configuration::claim_languages_edt(root);
        return;
    }
    match locus {
        XmlLocus::RootAttr { name } => {
            if let Some(a) = root.attr(name) {
                a.claimed.set(true);
            }
        }
        XmlLocus::PropElement { path, .. } => {
            let mut cur = root;
            for (i, seg) in path.iter().enumerate() {
                match cur.child(seg) {
                    Some(child) => {
                        if i + 1 == path.len() {
                            claim_element_for_codec(child, codec, version);
                            return;
                        }
                        cur = child;
                    }
                    None => return,
                }
            }
        }
    }
}

/// Claim самого элемента и (для контейнерных кодеков) его подструктуры. Должно
/// клеймить РОВНО то, что прочитает `decode_with_codec` (иначе leftover разойдётся с
/// decode и B1 для контейнеров не сработает).
fn claim_element_for_codec(el: &Element, codec: &Codec, version: FormatVersion) {
    match codec {
        // Designer-локализация: единый контейнер `<Tag>` с `<v8:item>`'ами. claim'им САМ
        // контейнер-узел (НЕ его прямой текст) и РОВНО первые два листа каждого item
        // (lang/content) с текстом — как читает строгий decode. EDT-локализация
        // (`LocalizedKeyVal`) — multi-sibling от корня, клеймится в `claim_locus`, сюда
        // НЕ доходит (см. from-root ветку ниже).
        Codec::LocalizedV8 => {
            el.claim();
            for item in &el.children {
                item.claim(); // <v8:item> — контейнер, его прямой текст не читается.
                              // Клеймим РОВНО первые два листа (lang, content) — столько и читает
                              // строгий decode_localized_v8. Любой третий/стрэй-лист остаётся
                              // НЕклеймнутым → попадёт в leftover (§1.0). Claim-проход и декодер
                              // согласованы: ни один не «глотает» больше другого (нет дрейфа).
                for leaf in item.children.iter().take(2) {
                    leaf.claim_with_text(); // <v8:lang>/<v8:content> + текст.
                }
            }
        }
        // Type-контейнер: claim'им host + детей РОВНО как читает type_codec::decode
        // (каждый <types>/<v8:Type> с текстом, каждый квалификатор-блок + его листья).
        // Лишнее остаётся неклеймнутым → leftover поймает (§1.0 B1).
        Codec::Type(dialect) => {
            el.claim();
            type_codec::claim(*dialect, el);
        }
        // Value-контейнер: claim host + xsi-атрибут + (для строки) <value>-ребёнок/текст
        // РОВНО как читает value_codec::decode. Лишнее остаётся неклеймнутым → leftover.
        Codec::Value(dialect) => {
            el.claim();
            value_codec::claim(*dialect, el);
        }
        // StyleValue-контейнер: claim host + вложенную структуру (EDT — inner `<value>`+
        // его листья; Designer — атрибуты+текст) РОВНО как читает style_value_codec::decode.
        Codec::StyleValue(dialect) => {
            el.claim();
            style_value_codec::claim(*dialect, el);
        }
        // ChoiceParameterLinks: host claimed; claim подструктуру как читает decode.
        Codec::ChoiceParameterLinks(dialect) => {
            el.claim();
            crate::choice_param_links::claim(*dialect, el);
        }
        // Designer RefList: host claimed; claim как читает decode_designer (field/item).
        Codec::RefList(dialect) => {
            el.claim();
            match dialect {
                crate::ref_list::RefListDialect::DesignerField => {
                    ref_list::claim_designer(el, false)
                }
                crate::ref_list::RefListDialect::DesignerItem => ref_list::claim_designer(el, true),
                crate::ref_list::RefListDialect::DesignerObject => {
                    ref_list::claim_designer_styled(el, crate::ref_list::DesignerStyle::Object)
                }
                crate::ref_list::RefListDialect::Edt => {} // claimed от корня в claim_locus.
            }
        }
        // Designer Characteristics: host claimed; claim как читает decode_designer.
        Codec::Characteristics(dialect) => {
            el.claim();
            if let crate::characteristics::CharacteristicsDialect::Designer = dialect {
                characteristics::claim_designer(el);
            }
        }
        // Designer CommonAttribute content: host claimed; claim `<xr:Item>`'ы как читает
        // decode_designer. EDT-вариант — multi-sibling от корня (claim в claim_locus).
        Codec::CommonAttributeContent(dialect) => {
            el.claim();
            if let crate::common_attribute_content::CommonAttributeContentDialect::Designer =
                dialect
            {
                common_attribute_content::claim_designer(el);
            }
        }
        // XdtoTypeRef: host claimed; claim листы/QName-текст/локальный xmlns как читает decode.
        Codec::XdtoTypeRef(dialect) => {
            el.claim();
            xdto_type_ref::claim(*dialect, el);
        }
        // Designer XdtoPackages: host claimed; claim `<xr:Item>`'ы как читает decode_designer.
        // EDT-вариант — multi-sibling от корня (claim в claim_locus, сюда не доходит).
        Codec::XdtoPackages(dialect) => {
            el.claim();
            if let crate::xdto_packages::XdtoPackagesDialect::Designer = dialect {
                xdto_packages::claim_designer(el);
            }
        }
        // Picture: host claimed; claim подструктуру как читает decode.
        Codec::PictureRef(dialect) => {
            el.claim();
            picture::claim(*dialect, el);
        }
        // LinkByType: host claimed; claim подструктуру как читает decode.
        Codec::LinkByType(dialect) => {
            el.claim();
            link_by_type::claim(*dialect, el);
        }
        // TransparentPixel (EDT): host claimed; claim x/y-листья как читает decode.
        Codec::TransparentPixel => {
            transparent_pixel::claim(el);
        }
        // Designer ChoiceParameters: host claimed; claim как читает decode_designer.
        Codec::ChoiceParameters(dialect) => {
            el.claim();
            if let crate::choice_parameters::ChoiceParametersDialect::Designer = dialect {
                choice_parameters::claim_designer(el);
            }
        }
        // HelpConst: host (`<help>`) claimed; claim фикс-подструктуру pages/lang.
        Codec::HelpConst => {
            el.claim();
            let _ = verify_help_const(el);
        }
        // Designer UsePurposes: host claimed; claim как читает decode.
        Codec::UsePurposesV8 => {
            el.claim();
            configuration::claim_use_purposes_designer(el);
        }
        // Designer form-ref UsePurposes-константа: та же v8:Value-структура, claim как decode.
        Codec::FormUsePurposesV8Const => {
            el.claim();
            configuration::claim_use_purposes_designer(el);
        }
        // MobileFunctionalities: host claimed; claim как читает decode (app_ns по диалекту).
        Codec::MobileFunctionalities(dialect) => {
            el.claim();
            let app_ns = match dialect {
                crate::configuration::ConfigDialect::Edt => "",
                crate::configuration::ConfigDialect::Designer => "app",
            };
            configuration::claim_mobile_functionalities(el, app_ns, version);
        }
        // EmptyValueList: пустой типизированный `<Tag xsi:type="xr:ValueList"/>`. claim'им САМ
        // узел + xsi:type-атрибут (детей/текста в witnessed-форме нет) — РОВНО как читает decode.
        Codec::EmptyValueList => {
            el.claim();
            if let Some(a) = el.attr("xsi:type") {
                a.claimed.set(true);
            }
        }
        // EDT extension-флаги: host claimed; claim xsi:type + листья-флаги как читает decode.
        Codec::ExtensionFlags => {
            el.claim();
            configuration::claim_extension_flags_edt(el);
        }
        // Presence/text-листы: сам узел + его текст (значение).
        Codec::BoolPresence
        | Codec::BoolText
        | Codec::EnumText
        | Codec::EnumSparse
        | Codec::IntText
        | Codec::PlainText
        | Codec::Shortcut
        | Codec::CompatibilityMode => {
            el.claim_with_text();
        }
        // from-root коды клеймятся в `claim_locus` от корня — сюда не доходят.
        // `LocalizedKeyVal` (EDT) — теперь тоже multi-sibling от корня (§1.0).
        Codec::LocalizedKeyVal
        | Codec::IrStandardAttributes(..)
        | Codec::UsePurposesConst
        | Codec::StdAttrs { .. }
        | Codec::StdTabularSections { .. }
        | Codec::ExchangePlanContent
        | Codec::PredefinedData
        | Codec::PredefinedDataCct
        | Codec::PredefinedDataCoa
        | Codec::ContainedObjects(_)
        | Codec::ConfigChildObjects(_)
        | Codec::LanguagesEntity => {}
    }
}

/// Преобразовать `PrefixMismatch` в типизированную ошибку кодека (B2).
fn prefix_err(local: &str, want: &str, got: &str) -> Decoded {
    let want_disp = if want.is_empty() { "(no prefix)" } else { want };
    let got_disp = if got.is_empty() { "(no prefix)" } else { got };
    Decoded::Error(format!(
        "element <{local}> has namespace prefix {got_disp:?}, expected {want_disp:?} (B2: \
         prefix is part of tag identity and must round-trip)"
    ))
}

/// Декодировать локализованную ячейку по кодеку в [`PropertyValue`].
pub(crate) fn decode_with_codec(
    codec: &Codec,
    located: Located<'_>,
    _expected: ValueKind,
    version: FormatVersion,
) -> Decoded {
    match codec {
        Codec::BoolPresence => match located {
            // EDT presence-bool: присутствие ⇒ true, текст ОБЯЗАН быть ровно "true"
            // (явный `false`/иной текст — не-каноническая форма, эмиттер её не
            // воспроизведёт ⇒ ошибка, а не Bool(false)→drop). Hardening из ревью.
            Located::Element(el) => {
                el.claim_text();
                if el.text == "true" {
                    Decoded::Present(PropertyValue::Bool(true))
                } else {
                    Decoded::Error(format!(
                        "BoolPresence element <{}> must have text \"true\" (presence-only), got {:?}",
                        el.local, el.text
                    ))
                }
            }
            Located::Attr(_) => Decoded::Error("BoolPresence on attribute is unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::BoolText => match located {
            Located::Element(el) => {
                el.claim_text();
                match el.text.as_str() {
                    "true" => Decoded::Present(PropertyValue::Bool(true)),
                    "false" => Decoded::Present(PropertyValue::Bool(false)),
                    other => Decoded::Error(format!("BoolText: not a bool literal: {other:?}")),
                }
            }
            Located::Attr(v) => match v {
                "true" => Decoded::Present(PropertyValue::Bool(true)),
                "false" => Decoded::Present(PropertyValue::Bool(false)),
                other => Decoded::Error(format!("BoolText: not a bool literal: {other:?}")),
            },
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::EnumText => match located {
            Located::Element(el) => {
                el.claim_text();
                Decoded::Present(PropertyValue::Enum(Token::new(el.text.clone())))
            }
            Located::Attr(v) => Decoded::Present(PropertyValue::Enum(Token::new(v.to_string()))),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        // Разрежённый enum: как EnumText, но present-узел ОБЯЗАН нести непустой литерал
        // (пустой токен — «отсутствует»-маркер дефолта; present-empty был бы неотличим →
        // §1.0-ошибка, не догадка).
        Codec::EnumSparse => match located {
            Located::Element(el) => {
                el.claim_text();
                if el.text.is_empty() {
                    Decoded::Error(format!(
                        "EnumSparse element <{}> must carry a non-empty literal (empty = the \
                         absent-marker default, never emitted; §1.0)",
                        el.local
                    ))
                } else {
                    Decoded::Present(PropertyValue::Enum(Token::new(el.text.clone())))
                }
            }
            Located::Attr(_) => Decoded::Error("EnumSparse on attribute is unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        // EDT extension-флаги корня расширения (host claimed locate'ом; см. configuration.rs).
        Codec::ExtensionFlags => match located {
            Located::Element(el) => configuration::decode_extension_flags_edt(el),
            Located::Attr(_) => Decoded::Error("ExtensionFlags on attribute is unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        // Режим совместимости: читаем ТЕКСТ и транслируем пер-форматную кодировку в канон
        // (EDT-dotted `8.3.20`). Designer-текст `Version8_3_20` → `8.3.20`; уже-dotted (или
        // не-версионный sentinel) — без изменений (see `compat_designer_to_canonical`).
        Codec::CompatibilityMode => match located {
            Located::Element(el) => {
                el.claim_text();
                Decoded::Present(PropertyValue::Enum(Token::new(
                    compat_designer_to_canonical(&el.text),
                )))
            }
            Located::Attr(v) => Decoded::Present(PropertyValue::Enum(Token::new(
                compat_designer_to_canonical(v),
            ))),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::IntText => match located {
            // Целое: текст элемента — десятичный i64. Строго (`parse::<i64>` отвергает
            // ведущие нули/знак-пробел/нечисло) → не угадываем (§1.0).
            Located::Element(el) => {
                el.claim_text();
                decode_int(&el.text)
            }
            Located::Attr(v) => decode_int(v),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::PlainText => match located {
            Located::Element(el) => {
                el.claim_text();
                Decoded::Present(PropertyValue::Str(el.text.clone()))
            }
            Located::Attr(v) => Decoded::Present(PropertyValue::Str(v.to_string())),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::LocalizedV8 => match located {
            Located::Element(container) => decode_localized_v8(container),
            Located::Attr(_) => Decoded::Error("LocalizedV8 on attribute is unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::Type(dialect) => match located {
            // Контейнер уже claimed `locate`'ом; type_codec разбирает детей. Present-
            // empty (`<type/>`/`<Type/>`) → `Type(TypeSpec{parts:[]})` (значение, не
            // Absent — host есть). Отсутствие host → Absent (поле опционально).
            Located::Element(container) => match type_codec::decode(*dialect, container) {
                Ok(v) => Decoded::Present(v),
                Err(e) => Decoded::Error(e),
            },
            Located::Attr(_) => Decoded::Error("Type codec on attribute is unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::Value(dialect) => match located {
            // Host уже claimed `locate`'ом; value_codec разбирает xsi-атрибут+скаляр.
            // Хост присутствует ВСЕГДА (это значение, не Absent); отсутствие host →
            // Absent (поле опционально, но в Constant required → host есть всегда).
            Located::Element(host) => match value_codec::decode(*dialect, host) {
                Ok(v) => Decoded::Present(v),
                Err(e) => Decoded::Error(e),
            },
            Located::Attr(_) => Decoded::Error("Value codec on attribute is unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::StyleValue(dialect) => match located {
            // Host claimed `locate`'ом; style_value_codec разбирает двухуровневую структуру
            // (EDT) / плоский `<Value>` (Designer). Host present всегда (это значение).
            Located::Element(host) => match style_value_codec::decode(*dialect, host) {
                Ok(v) => Decoded::Present(v),
                Err(e) => Decoded::Error(e),
            },
            Located::Attr(_) => {
                Decoded::Error("StyleValue codec on attribute is unsupported".into())
            }
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::ChoiceParameterLinks(dialect) => match located {
            Located::Element(host) => {
                use crate::choice_param_links::{decode_designer, decode_edt, LinksDialect};
                match dialect {
                    LinksDialect::Edt => decode_edt(host),
                    LinksDialect::Designer => decode_designer(host),
                }
            }
            Located::Attr(_) => {
                Decoded::Error("ChoiceParameterLinks on attribute unsupported".into())
            }
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::CommonAttributeContent(dialect) => match located {
            // Designer — single-container `<Content>` (host claimed `locate`'ом). Пустой
            // (self-closing) → `List([])` (значение, host есть). EDT — multi-sibling от
            // корня, разбирается ДО locus-dispatch (сюда не доходит).
            Located::Element(host) => {
                use crate::common_attribute_content::{
                    decode_designer, CommonAttributeContentDialect,
                };
                match dialect {
                    CommonAttributeContentDialect::Designer => decode_designer(host),
                    CommonAttributeContentDialect::Edt => Decoded::Error(
                        "CommonAttributeContent(Edt) must be decoded before locus dispatch".into(),
                    ),
                }
            }
            Located::Attr(_) => {
                Decoded::Error("CommonAttributeContent on attribute unsupported".into())
            }
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::HelpConst => match located {
            // EDT `<help>` присутствует ⇒ сверяем фикс-блок и даём Bool(true). Отсутствие
            // ⇒ Absent (дефолт Bool(false)). Любая иная структура → ошибка (§1.0).
            Located::Element(host) => match verify_help_const(host) {
                Ok(()) => Decoded::Present(PropertyValue::Bool(true)),
                Err(e) => Decoded::Error(e),
            },
            Located::Attr(_) => Decoded::Error("HelpConst on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::RefList(dialect) => match located {
            Located::Element(host) => {
                use crate::ref_list::{
                    decode_designer, decode_designer_styled, DesignerStyle, RefListDialect,
                };
                match dialect {
                    RefListDialect::DesignerField => decode_designer(host, false),
                    RefListDialect::DesignerItem => decode_designer(host, true),
                    RefListDialect::DesignerObject => {
                        decode_designer_styled(host, DesignerStyle::Object)
                    }
                    // EDT-вариант разбирается от корня в `decode_field`.
                    RefListDialect::Edt => {
                        Decoded::Error("RefList(Edt) must decode from root".into())
                    }
                }
            }
            Located::Attr(_) => Decoded::Error("RefList on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::Characteristics(dialect) => match located {
            Located::Element(host) => {
                use crate::characteristics::{decode_designer, CharacteristicsDialect};
                match dialect {
                    CharacteristicsDialect::Designer => decode_designer(host),
                    CharacteristicsDialect::Edt => {
                        Decoded::Error("Characteristics(Edt) must decode from root".into())
                    }
                }
            }
            Located::Attr(_) => Decoded::Error("Characteristics on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::PictureRef(dialect) => match located {
            Located::Element(host) => picture::decode(*dialect, host),
            Located::Attr(_) => Decoded::Error("PictureRef on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::Shortcut => match located {
            Located::Element(host) => shortcut::decode(host),
            Located::Attr(_) => Decoded::Error("Shortcut on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::TransparentPixel => match located {
            // EDT `<transparentPixel>`: присутствие ⇒ List([Int(x),Int(y)]) (sparse-листья,
            // см. `transparent_pixel`); отсутствие ⇒ Absent (дефолт — пустой List).
            Located::Element(host) => transparent_pixel::decode(host),
            Located::Attr(_) => Decoded::Error("TransparentPixel on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::LinkByType(dialect) => match located {
            Located::Element(host) => link_by_type::decode(*dialect, host),
            Located::Attr(_) => Decoded::Error("LinkByType on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::ChoiceParameters(dialect) => match located {
            Located::Element(host) => {
                use crate::choice_parameters::ChoiceParametersDialect;
                match dialect {
                    ChoiceParametersDialect::Designer => choice_parameters::decode_designer(host),
                    ChoiceParametersDialect::Edt => {
                        Decoded::Error("ChoiceParameters(Edt) must decode from root".into())
                    }
                }
            }
            Located::Attr(_) => Decoded::Error("ChoiceParameters on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::UsePurposesV8 => match located {
            Located::Element(host) => configuration::decode_use_purposes_designer(host),
            Located::Attr(_) => Decoded::Error("UsePurposesV8 on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        // Designer form-ref UsePurposes: v8:Value-структура декодится общим designer-разбором,
        // затем СВЕРЯЕТСЯ с константой [PersonalComputer, MobileDevice] (канон-литералы EDT) —
        // presence-канон Bool(true), как EDT UsePurposesConst. Отклонение — громко (§1.0).
        Codec::FormUsePurposesV8Const => match located {
            Located::Element(host) => match configuration::decode_use_purposes_designer(host) {
                Decoded::Present(PropertyValue::List(items)) => {
                    let want = ["PersonalComputer", "MobileDevice"];
                    let got: Vec<&str> = items
                        .iter()
                        .map(|it| match it {
                            PropertyValue::Str(s) => s.as_str(),
                            _ => "",
                        })
                        .collect();
                    if got == want {
                        Decoded::Present(PropertyValue::Bool(true))
                    } else {
                        Decoded::Error(format!(
                            "form UsePurposes: witnessed constant is {want:?}, got {got:?} \
                             (§1.0 — the FormRef spec carries a presence-marker, a deviating \
                             purpose set cannot be represented yet)"
                        ))
                    }
                }
                Decoded::Present(other) => Decoded::Error(format!(
                    "form UsePurposes must decode to a List, got {:?}",
                    other.kind()
                )),
                d => d,
            },
            Located::Attr(_) => {
                Decoded::Error("FormUsePurposesV8Const on attribute unsupported".into())
            }
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::MobileFunctionalities(dialect) => match located {
            Located::Element(host) => {
                let app_ns = match dialect {
                    crate::configuration::ConfigDialect::Edt => "",
                    crate::configuration::ConfigDialect::Designer => "app",
                };
                configuration::decode_mobile_functionalities(host, app_ns, version)
            }
            Located::Attr(_) => {
                Decoded::Error("MobileFunctionalities on attribute unsupported".into())
            }
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::XdtoTypeRef(dialect) => match located {
            // Host present ВСЕГДА (это значение); отсутствие → Absent (поле опционально).
            Located::Element(host) => match xdto_type_ref::decode(*dialect, host) {
                Ok(v) => Decoded::Present(v),
                Err(e) => Decoded::Error(e),
            },
            Located::Attr(_) => Decoded::Error("XdtoTypeRef on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::XdtoPackages(dialect) => match located {
            Located::Element(host) => match dialect {
                crate::xdto_packages::XdtoPackagesDialect::Designer => {
                    xdto_packages::decode_designer(host)
                }
                // EDT-вариант разбирается от корня в `decode_field`.
                crate::xdto_packages::XdtoPackagesDialect::Edt => {
                    Decoded::Error("XdtoPackages(Edt) must decode from root".into())
                }
            },
            Located::Attr(_) => Decoded::Error("XdtoPackages on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        Codec::EmptyValueList => match located {
            // Designer `<Tag xsi:type="xr:ValueList"/>` — ПУСТОЙ типизированный список. §1.0:
            // сверяем пустоту (нет детей/текста, xsi:type == xr:ValueList) и даём `List([])`
            // (движок сожмёт в дефолт `[]`). Непустой/иной xsi-тип → ошибка (не угадываем
            // layout непустого ValueList). Host present ⇒ значение; отсутствие → Absent.
            Located::Element(host) => decode_empty_value_list(host),
            Located::Attr(_) => Decoded::Error("EmptyValueList on attribute unsupported".into()),
            Located::PrefixMismatch { local, want, got } => prefix_err(&local, want, &got),
            Located::Absent => Decoded::Absent,
        },
        // from-root коды декодятся в `decode_field` от корня — сюда не доходят.
        // `LocalizedKeyVal` (EDT) — теперь тоже multi-sibling от корня (§1.0).
        Codec::LocalizedKeyVal
        | Codec::IrStandardAttributes(..)
        | Codec::UsePurposesConst
        | Codec::StdAttrs { .. }
        | Codec::StdTabularSections { .. }
        | Codec::ExchangePlanContent
        | Codec::PredefinedData
        | Codec::PredefinedDataCct
        | Codec::PredefinedDataCoa
        | Codec::ContainedObjects(_)
        | Codec::ConfigChildObjects(_)
        | Codec::LanguagesEntity => {
            Decoded::Error("from-root codec must be decoded from root, not a located cell".into())
        }
    }
}

/// Декодировать Designer `<Tag xsi:type="xr:ValueList"/>` (пустой типизированный список) в
/// канонический `List([])` (§1.0 witnessed-only: пустая форма). Непустой/иной xsi-тип → ошибка.
fn decode_empty_value_list(host: &Element) -> Decoded {
    if !host.children.is_empty() {
        return Decoded::Error(format!(
            "EmptyValueList <{}> must be empty (no children) — non-empty ValueList layout is \
             UNWITNESSED (§1.0, no guess)",
            host.local
        ));
    }
    if !host.text.is_empty() {
        return Decoded::Error(format!(
            "EmptyValueList <{}> must carry no text (§1.0)",
            host.local
        ));
    }
    match host.attr("xsi:type") {
        Some(a) if a.value == "xr:ValueList" => Decoded::Present(PropertyValue::List(Vec::new())),
        Some(a) => Decoded::Error(format!(
            "EmptyValueList <{}> xsi:type {:?} != \"xr:ValueList\" (§1.0)",
            host.local, a.value
        )),
        None => Decoded::Error(format!(
            "EmptyValueList <{}> missing xsi:type=\"xr:ValueList\" (§1.0)",
            host.local
        )),
    }
}

/// Сверить фикс-структуру EDT-`<help>` (`<pages><lang>ru</lang></pages>`) и claim'ить её
/// целиком. Любое отклонение → ошибка (§1.0). Хост уже claimed `locate`'ом.
fn verify_help_const(host: &Element) -> Result<(), String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("help: must be <help><pages><lang>ru</lang></pages></help>".into());
    }
    if host.children.len() != 1 {
        return Err("help: expected single <pages> child".into());
    }
    let pages = &host.children[0];
    if pages.local != "pages" || !pages.prefix.is_empty() || !pages.attrs.is_empty() {
        return Err("help: expected <pages> child".into());
    }
    pages.claim();
    if pages.children.len() != 1 {
        return Err("help: <pages> must have single <lang>".into());
    }
    let lang = &pages.children[0];
    if lang.local != "lang"
        || !lang.prefix.is_empty()
        || !lang.attrs.is_empty()
        || lang.text != "ru"
    {
        return Err("help: expected <lang>ru</lang>".into());
    }
    lang.claim_with_text();
    Ok(())
}

/// EDT-локализация (`synonym`/`toolTip`/…): MULTI-SIBLING разбор от КОРНЯ (§1.0).
///
/// EDT эмитит по ОДНОМУ `<tag>` на язык (двуязычный ERP: `<synonym><key>ru</key>
/// <value>…</value></synonym><synonym><key>en</key><value>…</value></synonym>`), в
/// отличие от Designer (единый `<Tag>` с несколькими `<v8:item>`). Раньше single-
/// element `locate` видел лишь ПЕРВЫЙ `<synonym>` → второй язык оставался
/// неклеймнутым (UnconsumedInput). Теперь читаем ВСЕ сиблинги `<tag>` под `root` в
/// ПОРЯДКЕ ИСТОЧНИКА, сплющивая их `<key>/<value>`-пары в один `Localized`-вектор
/// (order-preserving → byte-exact R). Внутри одного `<tag>` пар обычно одна, но
/// разбор допускает несколько чередующихся `<key>/<value>` (устойчивость).
///
/// Отсутствие тегов → `Localized([])` (сравнится с дефолтом-пустым и опустится —
/// как раньше при `Absent`). Пустой контейнер `<tag/>` (Designer-конвенция пустого)
/// в EDT-разрежённом корпусе не встречается, но обрабатывается (нет пар → ничего не
/// добавляет).
pub(crate) fn decode_localized_keyval_edt(root: &Element, tag: &str) -> Decoded {
    let mut pairs: Vec<(Lang, String)> = Vec::new();
    for container in root
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        if !container.attrs.is_empty() {
            return Decoded::Error(format!(
                "LocalizedKeyVal: <{tag}> must have no attributes (§1.0)"
            ));
        }
        container.claim();
        // Внутри контейнера — чередование <key>/<value> (обычно одна пара). Берём
        // позиционно; любой иной/непарный лист → ошибка (не тихий drop, §1.0).
        let mut iter = container.children.iter();
        while let Some(child) = iter.next() {
            if child.local != "key" || !child.prefix.is_empty() {
                return Decoded::Error(format!(
                    "LocalizedKeyVal: expected unprefixed <key>, got <{}>",
                    qname(child)
                ));
            }
            child.claim_with_text();
            let lang = child.text.clone();
            let val = match iter.next() {
                Some(v) if v.local == "value" && v.prefix.is_empty() => {
                    v.claim_with_text();
                    v.text.clone()
                }
                Some(v) => {
                    return Decoded::Error(format!(
                        "LocalizedKeyVal: expected unprefixed <value> after <key>, got <{}>",
                        qname(v)
                    ))
                }
                None => return Decoded::Error("LocalizedKeyVal: <key> without <value>".into()),
            };
            pairs.push((Lang::new(lang), val));
        }
    }
    Decoded::Present(PropertyValue::Localized(pairs))
}

/// Claim EDT-локализации (то же, что читает [`decode_localized_keyval_edt`]) — для
/// leftover. Клеймит каждый сиблинг `<tag>` + его `<key>/<value>` листья с текстом.
fn claim_localized_keyval_edt(root: &Element, tag: &str) {
    let _ = decode_localized_keyval_edt(root, tag);
}

/// Эмитировать EDT-локализацию: по ОДНОМУ сиблингу `<tag>` на язык-пару (в порядке
/// IR), каждый с одной парой `<key>lang</key><value>text</value>` — РОВНО как EDT
/// (сверено двуязычным ERP: N языков = N сиблингов `<synonym>`). Пустая локализация
/// → НОЛЬ узлов (разрежённый EDT дефолт-омиссию делает по `is_default` выше).
fn emit_localized_keyval_edt(tag: &str, value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    match value {
        PropertyValue::Localized(pairs) => {
            let mut out = Vec::with_capacity(pairs.len());
            for (lang, text) in pairs {
                let mut container = OutElement::branch("", tag);
                container.push(OutElement::leaf("", "key", lang.as_str().to_string()));
                container.push(OutElement::leaf("", "value", text.clone()));
                out.push(container);
            }
            Ok(out)
        }
        other => Err(format!(
            "LocalizedKeyVal expects Localized, got {:?}",
            other.kind()
        )),
    }
}

/// Designer `v8:item/(v8:lang,v8:content)` → `[(lang,content)…]`.
///
/// СТРОГО-ПОЗИЦИОННЫЙ разбор (как [`decode_localized_keyval_edt`], §1.0): каждый
/// `<v8:item>` ОБЯЗАН содержать РОВНО двух детей в порядке `<v8:lang>` затем
/// `<v8:content>` (оба с префиксом `v8`) и НИЧЕГО больше. Любой лишний/дублирующий/
/// иной/переставленный лист → типизированная ОШИБКА, а не тихий drop. Это закрывает
/// дыру §1.0: раньше `.find(...)` молча игнорировал стрэй-лист (`<v8:bogus>`) и второй
/// `<v8:lang>` — `leftover` оставался 0. Теперь claim-проход и декодер согласованы:
/// клеймятся РОВНО прочитанные `lang`/`content`, всё иное остаётся в `leftover`.
fn decode_localized_v8(container: &Element) -> Decoded {
    let mut pairs: Vec<(Lang, String)> = Vec::new();
    for item in &container.children {
        if item.local != "item" || item.prefix != "v8" {
            return Decoded::Error(format!(
                "LocalizedV8: expected <v8:item>, got <{}>",
                qname(item)
            ));
        }
        item.claim();
        // Ровно [v8:lang, v8:content] — ни больше, ни меньше, в этом порядке.
        let mut it = item.children.iter();
        let lang_el = match it.next() {
            Some(e) if e.local == "lang" && e.prefix == "v8" => e,
            Some(e) => {
                return Decoded::Error(format!(
                    "LocalizedV8: expected <v8:lang> as first item child, got <{}>",
                    qname(e)
                ))
            }
            None => return Decoded::Error("LocalizedV8: <v8:item> missing <v8:lang>".into()),
        };
        let content_el = match it.next() {
            Some(e) if e.local == "content" && e.prefix == "v8" => e,
            Some(e) => {
                return Decoded::Error(format!(
                    "LocalizedV8: expected <v8:content> after <v8:lang>, got <{}>",
                    qname(e)
                ))
            }
            None => return Decoded::Error("LocalizedV8: <v8:item> missing <v8:content>".into()),
        };
        // Любой третий ребёнок (стрэй-лист, дубль lang/content) → ошибка, не drop.
        if let Some(extra) = it.next() {
            return Decoded::Error(format!(
                "LocalizedV8: <v8:item> has unexpected extra child <{}> after \
                 <v8:lang>/<v8:content> (no silent drop — §1.0)",
                qname(extra)
            ));
        }
        lang_el.claim_with_text();
        content_el.claim_with_text();
        pairs.push((Lang::new(lang_el.text.clone()), content_el.text.clone()));
    }
    Decoded::Present(PropertyValue::Localized(pairs))
}

/// Разобрать десятичный i64 СТРОГО (как пишет 1С: без ведущих нулей/пробелов/знака-
/// плюс). Канон round-trip: `Int.to_string()` обязан дать ИСХОДНЫЙ текст, иначе R не
/// byte-exact — поэтому отвергаем не-каноническую форму (§1.0: не best-effort).
fn decode_int(s: &str) -> Decoded {
    match s.parse::<i64>() {
        Ok(n) if n.to_string() == s => Decoded::Present(PropertyValue::Int(n)),
        Ok(_) => Decoded::Error(format!(
            "IntText: {s:?} is not in canonical decimal form (no leading zeros/sign-space — §1.0)"
        )),
        Err(e) => Decoded::Error(format!("IntText: {s:?} is not a valid i64: {e}")),
    }
}

/// Полное имя тега элемента (`prefix:local` или `local`) для диагностики.
fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}

/// Local-name тега из локуса (последний сегмент `PropElement`) — для EDT-multi-node
/// кодеков (RefList), у которых тег сиблингов = имя поля. Пустой/RootAttr → `""`.
pub(crate) fn edt_list_tag(locus: &XmlLocus) -> &str {
    match locus {
        XmlLocus::PropElement { path, .. } => path.last().copied().unwrap_or(""),
        XmlLocus::RootAttr { .. } => "",
    }
}

/// Закодировать значение по локусу+кодеку и дописать дочерние [`OutElement`] в
/// приёмник. Большинство кодеков дают один элемент; локализованные — один контейнер.
pub(crate) fn encode_with_codec(
    locus: &XmlLocus,
    codec: &Codec,
    value: &PropertyValue,
    sink: &mut XmlSink,
    version: FormatVersion,
) -> Result<(), String> {
    // ВАРИАТИВНЫЙ standardAttributes видов с предопределённым набором атрибутов эмитит
    // НЕСКОЛЬКО узлов (EDT: N блоков), не адресуясь через локус-путь; обрабатываем до
    // разбора локуса.
    if let Codec::IrStandardAttributes(dialect, decl) = codec {
        for el in std_attrs_ir::emit(*dialect, decl, value, version)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // usePurposes-константа — два узла, value = presence-маркер Bool(true).
    if let Codec::UsePurposesConst = codec {
        match value {
            PropertyValue::Bool(true) => {
                for el in emit_use_purposes() {
                    sink.children.push(el);
                }
                Ok(())
            }
            PropertyValue::Bool(false) => Ok(()),
            other => Err(format!(
                "UsePurposesConst expects Bool, got {:?}",
                other.kind()
            )),
        }?;
        return Ok(());
    }
    // ОБОБЩЁННЫЙ standardAttributes — НЕСКОЛЬКО узлов (EDT: N блоков / Designer: 1 wrapper), от value.
    if let Codec::StdAttrs {
        dialect,
        decl,
        variant,
    } = codec
    {
        for el in std_attrs_generic::emit(*dialect, decl, *variant, value, version)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // standardTabularSections — multi-node (EDT: блок на ТЧ; Designer: одна обёртка;
    // пустой List ⇒ ноль узлов — блок опущен целиком).
    if let Codec::StdTabularSections { dialect, decl } = codec {
        for el in std_tabular_sections::emit(*dialect, decl, value, version)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // ExchangePlan content (EDT-only) — multi-sibling `<content>` от value.
    if let Codec::ExchangePlanContent = codec {
        for el in exchange_plan_content::emit_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // CommonAttribute content (EDT) — multi-sibling `<content>` от value. Designer —
    // single-container ниже (через locus-dispatch).
    if let Codec::CommonAttributeContent(
        crate::common_attribute_content::CommonAttributeContentDialect::Edt,
    ) = codec
    {
        for el in common_attribute_content::emit_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // EDT-локализация — MULTI-SIBLING от value: по ОДНОМУ `<tag>` на язык-пару (tag —
    // из локуса), в порядке IR. Пустая локализация → НОЛЬ узлов (разрежённый EDT
    // дефолт-омиссию делает выше по `is_default`). Designer — single-container ниже.
    if let Codec::LocalizedKeyVal = codec {
        for el in emit_localized_keyval_edt(edt_list_tag(locus), value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // RefList(Edt) — multi-sibling от value (tag — из локуса). Designer — single-container ниже.
    if let Codec::RefList(crate::ref_list::RefListDialect::Edt) = codec {
        for el in ref_list::emit_edt(edt_list_tag(locus), value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // Characteristics(Edt) — multi-sibling от value. Designer — single-container ниже.
    if let Codec::Characteristics(crate::characteristics::CharacteristicsDialect::Edt) = codec {
        for el in characteristics::emit_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // ChoiceParameters(Edt) — multi-sibling от value. Designer — single-container ниже.
    if let Codec::ChoiceParameters(crate::choice_parameters::ChoiceParametersDialect::Edt) = codec {
        for el in choice_parameters::emit_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // ChoiceParameterLinks(Edt) — multi-sibling от value (по узлу на связь). Designer —
    // single-container ниже (через locus-dispatch). Single-host — внутри std-attrs (OptCpl).
    if let Codec::ChoiceParameterLinks(crate::choice_param_links::LinksDialect::Edt) = codec {
        for el in choice_param_links::emit_edt_from_root(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // XdtoPackages(Edt) — multi-sibling `<xdtoPackages xsi:type><value>…` от value (tag —
    // из локуса). Designer — single-container ниже (через locus-dispatch).
    if let Codec::XdtoPackages(crate::xdto_packages::XdtoPackagesDialect::Edt) = codec {
        for el in xdto_packages::emit_edt(edt_list_tag(locus), value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // PredefinedDataCct (EDT-only) — один `<predefined>`-узел от value (List).
    if let Codec::PredefinedDataCct = codec {
        for el in predefined_cct::emit_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // PredefinedData (EDT-only) — один `<predefined>`-узел от value (List).
    if let Codec::PredefinedData = codec {
        for el in predefined::emit_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // PredefinedDataCoa (EDT-only) — один `<predefined>`-узел от value (List).
    if let Codec::PredefinedDataCoa = codec {
        for el in predefined_coa::emit_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // Configuration containedObjects — multi-node (EDT) / `<InternalInfo>` (Designer) от value.
    if let Codec::ContainedObjects(dialect) = codec {
        for el in configuration::emit_contained_objects(*dialect, value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // Configuration ChildObjects — multi-sibling (EDT) / `<ChildObjects>` (Designer) от value.
    if let Codec::ConfigChildObjects(dialect) = codec {
        for el in configuration::emit_child_objects(*dialect, value)? {
            sink.children.push(el);
        }
        return Ok(());
    }
    // Configuration languages — EDT-only inline-сущности (multi-node) от value.
    if let Codec::LanguagesEntity = codec {
        for el in configuration::emit_languages_edt(value)? {
            sink.children.push(el);
        }
        return Ok(());
    }

    // Имя тега листа = последний сегмент пути (для PropElement); ns = locus.ns.
    let (tag, ns) = match locus {
        XmlLocus::PropElement { path, ns } => {
            let last = path.last().copied().ok_or("empty PropElement path")?;
            (last, *ns)
        }
        XmlLocus::RootAttr { .. } => {
            return Err("encode into RootAttr is handled by object frame, not engine".into())
        }
    };

    match codec {
        Codec::BoolPresence => match value {
            // EDT: presence-only, текст всегда "true". false должно было быть дефолтом
            // (re-sparsified) и сюда не дойти; если дошло — это не presence-bool.
            PropertyValue::Bool(true) => {
                sink.children.push(OutElement::leaf(ns, tag, "true"));
                Ok(())
            }
            PropertyValue::Bool(false) => Ok(()), // не эмитим (presence==false)
            other => Err(format!("BoolPresence expects Bool, got {:?}", other.kind())),
        },
        Codec::BoolText => match value {
            PropertyValue::Bool(b) => {
                sink.children
                    .push(OutElement::leaf(ns, tag, if *b { "true" } else { "false" }));
                Ok(())
            }
            other => Err(format!("BoolText expects Bool, got {:?}", other.kind())),
        },
        Codec::EnumText => match value {
            PropertyValue::Enum(tok) => {
                sink.children
                    .push(OutElement::leaf(ns, tag, tok.as_str().to_string()));
                Ok(())
            }
            other => Err(format!("EnumText expects Enum, got {:?}", other.kind())),
        },
        // Разрежённый enum: пустой токен («отсутствует»-маркер) → НОЛЬ узлов даже у
        // плотного (Designer) райтера; непустой → обычный текст-лист.
        Codec::EnumSparse => match value {
            PropertyValue::Enum(tok) => {
                if !tok.as_str().is_empty() {
                    sink.children
                        .push(OutElement::leaf(ns, tag, tok.as_str().to_string()));
                }
                Ok(())
            }
            other => Err(format!("EnumSparse expects Enum, got {:?}", other.kind())),
        },
        // EDT extension-флаги корня расширения — один структурный узел от value.
        Codec::ExtensionFlags => {
            sink.children
                .push(configuration::emit_extension_flags_edt(ns, tag, value)?);
            Ok(())
        }
        // Режим совместимости: канон (EDT-dotted `8.3.20`) → пер-форматная кодировка. Для
        // Designer-локуса эмитим `Version8_3_20`; уже-Version (или не-версионный sentinel) —
        // без изменений (see `compat_canonical_to_designer`). EDT-коннектор использует EnumText
        // (verbatim dotted) — сюда доходит лишь Designer-проекция этого поля.
        Codec::CompatibilityMode => match value {
            PropertyValue::Enum(tok) => {
                sink.children.push(OutElement::leaf(
                    ns,
                    tag,
                    compat_canonical_to_designer(tok.as_str()),
                ));
                Ok(())
            }
            other => Err(format!(
                "CompatibilityMode expects Enum, got {:?}",
                other.kind()
            )),
        },
        Codec::IntText => match value {
            // Десятичный литерал i64 (каноничен — `to_string` без ведущих нулей).
            PropertyValue::Int(n) => {
                sink.children.push(OutElement::leaf(ns, tag, n.to_string()));
                Ok(())
            }
            other => Err(format!("IntText expects Int, got {:?}", other.kind())),
        },
        Codec::PlainText => match value {
            PropertyValue::Str(s) => {
                // Конвенция 1С: ПУСТОЙ текстовый элемент эмитится самозакрывающимся
                // (`<Comment/>`), а не `<Comment></Comment>` — сверено по Designer-
                // корпусу (544/556 пустых `<Comment/>`). Непустой → обычный лист
                // `<Comment>текст</Comment>`. Для EDT путь недостижим: разрежённая
                // проекция дефолтный (пустой) comment не эмитит вовсе.
                if s.is_empty() {
                    sink.children.push(OutElement::self_closing(ns, tag));
                } else {
                    sink.children.push(OutElement::leaf(ns, tag, s.clone()));
                }
                Ok(())
            }
            other => Err(format!("PlainText expects Str, got {:?}", other.kind())),
        },
        Codec::LocalizedV8 => match value {
            PropertyValue::Localized(pairs) => {
                // Пустая локализованная строка → самозакрывающийся `<Tag/>` (Designer
                // DENSE так эмитит пустые ListPresentation/Explanation; сверено корпусом).
                if pairs.is_empty() {
                    sink.children.push(OutElement::self_closing(ns, tag));
                } else {
                    let mut container = OutElement::branch(ns, tag);
                    for (lang, content) in pairs {
                        let mut item = OutElement::branch("v8", "item");
                        item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
                        item.push(OutElement::leaf("v8", "content", content.clone()));
                        container.push(item);
                    }
                    sink.children.push(container);
                }
                Ok(())
            }
            other => Err(format!(
                "LocalizedV8 expects Localized, got {:?}",
                other.kind()
            )),
        },
        Codec::Type(dialect) => match value {
            // Host-имя/ns берём из локуса (`<type>`/`<Type>`/`<valueType>`); детей и
            // self-closing-для-пустого решает type_codec (sparse/dense per dialect).
            PropertyValue::Type(spec) => {
                let host = type_codec::encode(*dialect, ns, tag, spec)?;
                sink.children.push(host);
                Ok(())
            }
            other => Err(format!("Type codec expects Type, got {:?}", other.kind())),
        },
        Codec::Value(dialect) => match value {
            // Host-имя/ns — из локуса (`<minValue>`/`<MinValue>`); xsi-атрибут и
            // self-closing/`<value>`/текст решает value_codec (per dialect).
            PropertyValue::Value(spec) => {
                let host = value_codec::encode(*dialect, ns, tag, spec)?;
                sink.children.push(host);
                Ok(())
            }
            other => Err(format!("Value codec expects Value, got {:?}", other.kind())),
        },
        Codec::StyleValue(dialect) => match value {
            PropertyValue::StyleValue(spec) => {
                let host = style_value_codec::encode(*dialect, ns, tag, spec)?;
                sink.children.push(host);
                Ok(())
            }
            other => Err(format!(
                "StyleValue codec expects StyleValue, got {:?}",
                other.kind()
            )),
        },
        Codec::ChoiceParameterLinks(dialect) => {
            use crate::choice_param_links::{encode_designer, encode_edt, LinksDialect};
            let host = match dialect {
                LinksDialect::Edt => encode_edt(ns, tag, value)?,
                LinksDialect::Designer => encode_designer(ns, tag, value)?,
            };
            sink.children.push(host);
            Ok(())
        }
        Codec::RefList(dialect) => {
            use crate::ref_list::{
                emit_designer, emit_designer_styled, DesignerStyle, RefListDialect,
            };
            let host = match dialect {
                RefListDialect::DesignerField => emit_designer(ns, tag, false, value)?,
                RefListDialect::DesignerItem => emit_designer(ns, tag, true, value)?,
                RefListDialect::DesignerObject => {
                    emit_designer_styled(ns, tag, DesignerStyle::Object, value)?
                }
                // EDT — multi-node, эмитится выше (до разбора локуса).
                RefListDialect::Edt => {
                    return Err("RefList(Edt) must be emitted before locus dispatch".into())
                }
            };
            sink.children.push(host);
            Ok(())
        }
        Codec::Characteristics(dialect) => {
            use crate::characteristics::CharacteristicsDialect;
            match dialect {
                CharacteristicsDialect::Designer => {
                    sink.children
                        .push(characteristics::emit_designer(ns, tag, value)?);
                    Ok(())
                }
                CharacteristicsDialect::Edt => {
                    Err("Characteristics(Edt) must be emitted before locus dispatch".into())
                }
            }
        }
        Codec::CommonAttributeContent(dialect) => {
            use crate::common_attribute_content::CommonAttributeContentDialect;
            match dialect {
                CommonAttributeContentDialect::Designer => {
                    sink.children
                        .push(common_attribute_content::emit_designer(ns, tag, value)?);
                    Ok(())
                }
                CommonAttributeContentDialect::Edt => {
                    Err("CommonAttributeContent(Edt) must be emitted before locus dispatch".into())
                }
            }
        }
        Codec::PictureRef(dialect) => {
            sink.children
                .push(picture::encode(*dialect, ns, tag, value)?);
            Ok(())
        }
        Codec::Shortcut => {
            sink.children.push(shortcut::encode(ns, tag, value)?);
            Ok(())
        }
        Codec::TransparentPixel => {
            sink.children
                .push(transparent_pixel::encode(ns, tag, value)?);
            Ok(())
        }
        Codec::LinkByType(dialect) => {
            sink.children
                .push(link_by_type::encode(*dialect, ns, tag, value)?);
            Ok(())
        }
        Codec::ChoiceParameters(dialect) => {
            use crate::choice_parameters::ChoiceParametersDialect;
            match dialect {
                ChoiceParametersDialect::Designer => {
                    sink.children
                        .push(choice_parameters::emit_designer(ns, tag, value)?);
                    Ok(())
                }
                ChoiceParametersDialect::Edt => {
                    Err("ChoiceParameters(Edt) must be emitted before locus dispatch".into())
                }
            }
        }
        Codec::HelpConst => match value {
            // EDT-only: presence-маркер Bool(true) → фикс-блок `<help><pages><lang>ru</lang>
            // </pages></help>`. Bool(false) сюда не доходит (re-sparsify дефолта).
            PropertyValue::Bool(true) => {
                sink.children.push(help_const_block(ns, tag));
                Ok(())
            }
            PropertyValue::Bool(false) => Ok(()),
            other => Err(format!("HelpConst expects Bool, got {:?}", other.kind())),
        },
        Codec::UsePurposesV8 => {
            sink.children
                .push(configuration::emit_use_purposes_designer(ns, tag, value)?);
            Ok(())
        }
        // Designer form-ref UsePurposes-константа: presence-маркер Bool(true) → const-блок
        // (канон-литералы транслируются в Designer-спеллинг общим эмиттером).
        Codec::FormUsePurposesV8Const => match value {
            PropertyValue::Bool(true) => {
                let items = PropertyValue::List(vec![
                    PropertyValue::Str("PersonalComputer".into()),
                    PropertyValue::Str("MobileDevice".into()),
                ]);
                sink.children
                    .push(configuration::emit_use_purposes_designer(ns, tag, &items)?);
                Ok(())
            }
            PropertyValue::Bool(false) => Ok(()),
            other => Err(format!(
                "FormUsePurposesV8Const expects Bool, got {:?}",
                other.kind()
            )),
        },
        Codec::MobileFunctionalities(dialect) => {
            let app_ns = match dialect {
                crate::configuration::ConfigDialect::Edt => "",
                crate::configuration::ConfigDialect::Designer => "app",
            };
            sink.children
                .push(configuration::emit_mobile_functionalities(
                    ns, tag, app_ns, value, version,
                )?);
            Ok(())
        }
        Codec::XdtoTypeRef(dialect) => {
            sink.children
                .push(xdto_type_ref::encode(*dialect, ns, tag, value)?);
            Ok(())
        }
        Codec::XdtoPackages(dialect) => match dialect {
            crate::xdto_packages::XdtoPackagesDialect::Designer => {
                sink.children
                    .push(xdto_packages::emit_designer(ns, tag, value)?);
                Ok(())
            }
            // EDT — multi-node, эмитится выше (до разбора локуса).
            crate::xdto_packages::XdtoPackagesDialect::Edt => {
                Err("XdtoPackages(Edt) must be emitted before locus dispatch".into())
            }
        },
        Codec::EmptyValueList => match value {
            // Designer DENSE: пустой список → `<Tag xsi:type="xr:ValueList"/>` (§1.0 witnessed:
            // непустой список — UNWITNESSED, ошибка). EDT/cf этот кодек не используют (List не
            // деривируем в EDT; cf hand-written), поэтому доходит лишь Designer-путь.
            PropertyValue::List(items) if items.is_empty() => {
                sink.children
                    .push(OutElement::self_closing(ns, tag).attr("xsi:type", "xr:ValueList"));
                Ok(())
            }
            PropertyValue::List(_) => Err(format!(
                "EmptyValueList <{tag}>: non-empty ValueList is UNWITNESSED — no structural \
                 encoding (§1.0, no guess)"
            )),
            other => Err(format!(
                "EmptyValueList expects List, got {:?}",
                other.kind()
            )),
        },
        // multi-node/from-root коды эмитятся выше (до разбора локуса) — сюда не доходят.
        // `LocalizedKeyVal` (EDT) — теперь тоже multi-sibling от корня (§1.0).
        Codec::LocalizedKeyVal
        | Codec::IrStandardAttributes(..)
        | Codec::UsePurposesConst
        | Codec::StdAttrs { .. }
        | Codec::StdTabularSections { .. }
        | Codec::ExchangePlanContent
        | Codec::PredefinedData
        | Codec::PredefinedDataCct
        | Codec::PredefinedDataCoa
        | Codec::ContainedObjects(_)
        | Codec::ConfigChildObjects(_)
        | Codec::LanguagesEntity => {
            Err("multi-node codec must be emitted before locus dispatch".into())
        }
    }
}

/// Фикс-блок EDT-only `<help><pages><lang>ru</lang></pages></help>` (byte-exact;
/// сверено: 46/46 вхождений побайтово идентичны во всём IR-корпусе).
fn help_const_block(ns: &str, tag: &str) -> OutElement {
    let mut help = OutElement::branch(ns, tag);
    let mut pages = OutElement::branch("", "pages");
    pages.push(OutElement::leaf("", "lang", "ru"));
    help.push(pages);
    help
}

/// Константа usePurposes: РОВНО два значения в фикс-порядке.
const USE_PURPOSES: &[&str] = &["PersonalComputer", "MobileDevice"];

/// Декодировать `usePurposes`-константу от КОРНЯ источника (props_root формы): два
/// подряд `<usePurposes>` с фикс-значениями. Claim'ит оба. Любое отклонение → ошибка.
pub(crate) fn decode_use_purposes(root: &Element) -> Decoded {
    let blocks: Vec<&Element> = root
        .children
        .iter()
        .filter(|c| c.local == "usePurposes" && c.prefix.is_empty())
        .collect();
    if blocks.len() != USE_PURPOSES.len() {
        return Decoded::Error(format!(
            "usePurposes: expected {} <usePurposes>, found {}",
            USE_PURPOSES.len(),
            blocks.len()
        ));
    }
    for (b, want) in blocks.iter().zip(USE_PURPOSES) {
        if !b.attrs.is_empty() || !b.children.is_empty() || b.text != *want {
            return Decoded::Error(format!("usePurposes: expected {want:?}, got {:?}", b.text));
        }
        b.claim_with_text();
    }
    Decoded::Present(PropertyValue::Bool(true))
}

/// Claim usePurposes-константу (если совпала) — для leftover.
fn claim_use_purposes(root: &Element) {
    let _ = decode_use_purposes(root);
}

/// Эмитить usePurposes-константу: два узла `<usePurposes>…`.
fn emit_use_purposes() -> Vec<OutElement> {
    USE_PURPOSES
        .iter()
        .map(|v| OutElement::leaf("", "usePurposes", v.to_string()))
        .collect()
}
