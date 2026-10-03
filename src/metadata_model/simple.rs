//! Simple objects: Constant, DefinedType, SessionParameter, CommonAttribute,
//! FunctionalOption, FunctionalOptionsParameter, EventSubscription,
//! ScheduledJob, SettingsStorage, FilterCriterion, Language.
//!
//! Every row is `{1,<payload>,0}` (or, for objects with child collections,
//! `{1,<payload>,<collection count>,<collection>...}`).

#[path = "simple_export.rs"]
pub(crate) mod export;

use anyhow::{Result, anyhow, bail};

use super::attribute::{attribute_body, bool_prop, localized_prop, typed_header};
use super::brace::{Brace, NIL_UUID};
use super::types::{form_uuid, metadata_ref, object_uuid, type_pattern};
use super::xml::Element;
use super::{DescriptorContext, ObjectXml, md_base, native_text, not_yet};
use crate::brace_list;

pub fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    match object.kind {
        "Language" => language(object),
        "Constant" => constant(object, context),
        "DefinedType" => defined_type(object, context),
        "SessionParameter" => session_parameter(object, context),
        "CommonAttribute" => common_attribute(object, context),
        "FunctionalOption" => functional_option(object, context),
        "FunctionalOptionsParameter" => functional_options_parameter(object, context),
        "EventSubscription" => event_subscription(object, context),
        "ScheduledJob" => scheduled_job(object, context),
        "SettingsStorage" => settings_storage(object, context),
        "FilterCriterion" => filter_criterion(object, context),
        _ => Err(not_yet(object)),
    }
}

/// `{1,<payload>,0}`.
fn row(payload: Brace) -> Brace {
    brace_list![Brace::num(1), payload, Brace::num(0)]
}

/// The (TypeId, ValueId) of the object's own generated type of a category
/// (`Manager`, `ValueManager`, `ValueKey`, `List`, `DefinedType`, ...).
pub fn generated_ids(element: &Element, category: &str) -> Result<(Brace, Brace)> {
    let generated = element
        .child("InternalInfo")
        .and_then(|info| {
            info.children_named("GeneratedType")
                .find(|generated| generated.attr("category") == Some(category))
        })
        .ok_or_else(|| anyhow!("no generated type of category {category}"))?;
    Ok((
        Brace::uuid(generated.child_text("TypeId").unwrap_or(NIL_UUID).trim()),
        Brace::uuid(generated.child_text("ValueId").unwrap_or(NIL_UUID).trim()),
    ))
}

/// A `Module.Method` handler (`CommonModule.X.Proc`) -> (module uuid,
/// method name); an empty handler is the nil uuid and `""`.
fn handler(text: &str, context: &DescriptorContext) -> Result<(Brace, Brace)> {
    let text = text.trim();
    if text.is_empty() {
        return Ok((Brace::nil_uuid(), Brace::str("")));
    }
    let (module, method) = text
        .rsplit_once('.')
        .ok_or_else(|| anyhow!("bad handler {text}"))?;
    Ok((
        Brace::uuid(&object_uuid(module, context)?),
        Brace::str(method),
    ))
}

fn uuid_or_nil(full_name: Option<&str>, context: &DescriptorContext) -> Result<Brace> {
    match full_name.map(str::trim).filter(|name| !name.is_empty()) {
        None => Ok(Brace::nil_uuid()),
        Some(name) => Ok(Brace::uuid(&object_uuid(name, context)?)),
    }
}

fn enumerated(value: Option<&str>, name: &str, table: &[(&str, i64)]) -> Result<Brace> {
    let value = value.unwrap_or_default().trim();
    table
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map(|(_, code)| Brace::num(*code))
        .ok_or_else(|| anyhow!("unknown {name} {value:?}"))
}

/// `{1,{0,<md base>,"<LanguageCode>"},0}`
fn language(object: &ObjectXml<'_>) -> Result<Brace> {
    let properties = object.properties()?;
    Ok(row(brace_list![
        Brace::num(0),
        md_base(&object.uuid, properties),
        Brace::str(object.prop_text("LanguageCode")?),
    ]))
}

/// `{1,{16,<attribute body>,<Manager ids>,<ValueManager ids>,
/// <DataLockControlMode>,<UseStandardCommands>,<ExtendedPresentation>,
/// <Explanation>,<DefaultForm>,0,<DataHistory>,<ValueKey ids>,
/// <UpdateDataHistoryImmediatelyAfterWrite>,
/// <ExecuteAfterWriteDataHistoryVersionProcessing>},0}`
///
/// Slot 11 is `1` on six ERP УХ constants (`ВсегдаКонтролироватьБалансРучныхОпераций`,
/// `ДополнительныеЯзыкиВыводаОтчета`, `НастройкиКолонтитуловПоУмолчанию`,
/// `ПутьККаталогуИмпорта`, `СрокОплатыПокупателей`, `СрокОплатыПоставщикам`)
/// and `0` on the other 3 354 constants of the four corpora; nothing in the
/// XML tells them apart, so it is written as `0`.
fn constant(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    let (manager_type, manager_value) = generated_ids(object.element, "Manager")?;
    let (value_manager_type, value_manager_value) = generated_ids(object.element, "ValueManager")?;
    let (key_type, key_value) = generated_ids(object.element, "ValueKey")?;
    Ok(row(brace_list![
        Brace::num(16),
        attribute_body(&object.uuid, properties, context)?,
        manager_type,
        manager_value,
        value_manager_type,
        value_manager_value,
        data_lock_control_mode(properties.child_text("DataLockControlMode"))?,
        bool_prop(properties, "UseStandardCommands")?,
        localized_prop(properties, "ExtendedPresentation"),
        localized_prop(properties, "Explanation"),
        Brace::uuid(&form_uuid(
            properties.child_text("DefaultForm").unwrap_or_default(),
            context
        )?),
        Brace::num(0),
        data_history(properties.child_text("DataHistory"))?,
        key_type,
        key_value,
        bool_prop(properties, "UpdateDataHistoryImmediatelyAfterWrite")?,
        bool_prop(properties, "ExecuteAfterWriteDataHistoryVersionProcessing")?,
    ]))
}

/// `Automatic` 0, `Managed` 1.
pub fn data_lock_control_mode(value: Option<&str>) -> Result<Brace> {
    enumerated(
        Some(value.unwrap_or("Automatic")),
        "DataLockControlMode",
        &[("Automatic", 0), ("Managed", 1)],
    )
}

/// `DontUse` 0, `Use` 1.
pub fn data_history(value: Option<&str>) -> Result<Brace> {
    enumerated(
        Some(value.unwrap_or("DontUse")),
        "DataHistory",
        &[("DontUse", 0), ("Use", 1)],
    )
}

/// `{1,{0,<DefinedType ids>,<md base>,<type pattern>},0}`
fn defined_type(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    let (type_id, value_id) = generated_ids(object.element, "DefinedType")?;
    Ok(row(brace_list![
        Brace::num(0),
        type_id,
        value_id,
        md_base(&object.uuid, properties),
        type_pattern(properties.child("Type"), context)?,
    ]))
}

/// `{1,{1,{2,<md base>,<type pattern>}},0}`
fn session_parameter(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    Ok(row(brace_list![
        Brace::num(1),
        typed_header(&object.uuid, properties, context)?,
    ]))
}

/// `{1,{5,<attribute body>,<content>,<Indexing>,<FullTextSearch>,
/// <DataSeparation>,<AutoUse>,{1,<DataSeparationValue>},{1,<DataSeparationUse>},
/// {1,<ConditionalSeparation>},<UsersSeparation>,<AuthenticationSeparation>,
/// <SeparatedDataUse>,<ConfigurationExtensionsSeparation>,<DataHistory>},0}`
///
/// The content is `{3,<count>,<object uuid>,{2,<use>,<conditional separation>},...}`.
fn common_attribute(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    let mut content = vec![Brace::num(3), Brace::num(0)];
    let mut count = 0;
    if let Some(items) = properties.child("Content") {
        for item in items.children_named("Item") {
            count += 1;
            content.push(Brace::uuid(&object_uuid(
                item.child_text("Metadata").unwrap_or_default(),
                context,
            )?));
            content.push(brace_list![
                Brace::num(2),
                enumerated(
                    item.child_text("Use"),
                    "content Use",
                    &[("Auto", 0), ("Use", 1), ("DontUse", 2)]
                )?,
                uuid_or_nil(item.child_text("ConditionalSeparation"), context)?,
            ]);
        }
    }
    content[1] = Brace::num(count);
    let separation = |name: &str| -> Result<Brace> {
        enumerated(
            properties.child_text(name),
            name,
            &[("DontUse", 0), ("Separate", 1)],
        )
    };
    Ok(row(brace_list![
        Brace::num(5),
        attribute_body(&object.uuid, properties, context)?,
        Brace::List(content),
        indexing(properties.child_text("Indexing"))?,
        enumerated(
            properties.child_text("FullTextSearch"),
            "FullTextSearch",
            &[("DontUse", 0), ("Use", 1)]
        )?,
        enumerated(
            properties.child_text("DataSeparation"),
            "DataSeparation",
            &[("Separate", 0), ("DontUse", 1)]
        )?,
        enumerated(
            properties.child_text("AutoUse"),
            "AutoUse",
            &[("Use", 0), ("DontUse", 1)]
        )?,
        brace_list![
            Brace::num(1),
            uuid_or_nil(properties.child_text("DataSeparationValue"), context)?
        ],
        brace_list![
            Brace::num(1),
            uuid_or_nil(properties.child_text("DataSeparationUse"), context)?
        ],
        brace_list![
            Brace::num(1),
            uuid_or_nil(properties.child_text("ConditionalSeparation"), context)?
        ],
        separation("UsersSeparation")?,
        separation("AuthenticationSeparation")?,
        enumerated(
            properties.child_text("SeparatedDataUse"),
            "SeparatedDataUse",
            &[("Independently", 0), ("IndependentlyAndSimultaneously", 1)]
        )?,
        separation("ConfigurationExtensionsSeparation")?,
        data_history(properties.child_text("DataHistory"))?,
    ]))
}

/// `DontIndex` 0, `Index` 1, `IndexWithAdditionalOrder` 2.
pub fn indexing(value: Option<&str>) -> Result<Brace> {
    enumerated(
        Some(value.unwrap_or("DontIndex")),
        "Indexing",
        &[
            ("DontIndex", 0),
            ("Index", 1),
            ("IndexWithAdditionalOrder", 2),
        ],
    )
}

/// `{1,{2,<md base>,<Location>,{0,<count>,<item>...},<PrivilegedGetMode>},0}`,
/// an item being `{"#",3ea29ea5-...,{0,<metadata ref>}}`.
fn functional_option(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    const CONTENT_ITEM_TYPE: &str = "3ea29ea5-66f6-4e3b-8595-d8940db766a2";
    let properties = object.properties()?;
    let mut content = vec![Brace::num(0), Brace::num(0)];
    if let Some(items) = properties.child("Content") {
        for item in items.children_named("Object") {
            content.push(brace_list![
                Brace::str("#"),
                Brace::uuid(CONTENT_ITEM_TYPE),
                brace_list![Brace::num(0), metadata_ref(&item.text, context)?],
            ]);
        }
    }
    content[1] = Brace::num((content.len() - 2) as i64);
    Ok(row(brace_list![
        Brace::num(2),
        md_base(&object.uuid, properties),
        uuid_or_nil(properties.child_text("Location"), context)?,
        Brace::List(content),
        bool_prop(properties, "PrivilegedGetMode")?,
    ]))
}

/// `{1,{0,<md base>,{0,<count>,<metadata ref>...}},0}` (`{0}` when empty)
fn functional_options_parameter(
    object: &ObjectXml<'_>,
    context: &DescriptorContext,
) -> Result<Brace> {
    let properties = object.properties()?;
    let mut uses = vec![Brace::num(0), Brace::num(0)];
    if let Some(items) = properties.child("Use") {
        for item in items.children_named("Item") {
            uses.push(metadata_ref(&item.text, context)?);
        }
    }
    uses[1] = Brace::num((uses.len() - 2) as i64);
    if uses.len() == 2 {
        // No corpus parameter is empty; the older strict codec
        // (`compiler::families::simple`) reads an empty list as bare `{0}`.
        uses.truncate(1);
    }
    Ok(row(brace_list![
        Brace::num(0),
        md_base(&object.uuid, properties),
        Brace::List(uses),
    ]))
}

/// The stored spelling of a subscription event: `<name>_<Russian name>`.
fn event_name(event: &str) -> Result<String> {
    let russian = match event {
        "BeforeWrite" => "ПередЗаписью",
        "OnWrite" => "ПриЗаписи",
        "BeforeDelete" => "ПередУдалением",
        "Filling" => "ОбработкаЗаполнения",
        "FillCheckProcessing" => "ОбработкаПроверкиЗаполнения",
        "OnCopy" => "ПриКопировании",
        "OnSetNewCode" => "ПриУстановкеНовогоКода",
        "OnSetNewNumber" => "ПриУстановкеНовогоНомера",
        "Posting" => "ОбработкаПроведения",
        "UndoPosting" => "ОбработкаУдаленияПроведения",
        "FormGetProcessing" => "ОбработкаПолученияФормы",
        "ChoiceDataGetProcessing" => "ОбработкаПолученияДанныхВыбора",
        "PresentationFieldsGetProcessing" => "ОбработкаПолученияПолейПредставления",
        "PresentationGetProcessing" => "ОбработкаПолученияПредставления",
        "OnReceiveDataFromMaster" => "ПриПолученииДанныхОтГлавного",
        "OnReceiveDataFromSlave" => "ПриПолученииДанныхОтПодчиненного",
        "OnSendDataToMaster" => "ПриОтправкеДанныхГлавному",
        "OnSendDataToSlave" => "ПриОтправкеДанныхПодчиненному",
        "OnSendNodeDataToSlave" => "ПриОтправкеДанныхУзлаПодчиненному",
        "" => return Ok(String::new()),
        other => bail!("unknown subscription event {other}"),
    };
    Ok(format!("{event}_{russian}"))
}

/// `{1,{1,<md base>,<source pattern>,"<event>",<handler module>,"<method>"},0}`
fn event_subscription(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    let (module, method) = handler(
        properties.child_text("Handler").unwrap_or_default(),
        context,
    )?;
    Ok(row(brace_list![
        Brace::num(1),
        md_base(&object.uuid, properties),
        type_pattern(properties.child("Source"), context)?,
        Brace::str(event_name(
            properties.child_text("Event").unwrap_or_default().trim()
        )?),
        module,
        method,
    ]))
}

/// `{1,{2,<md base>,"<Key>","<Description>",<Use>,<Predefined>,<module>,
/// "<method>",<RestartCountOnFailure>,<RestartIntervalOnFailure>},0}`
fn scheduled_job(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    let (module, method) = handler(
        properties.child_text("MethodName").unwrap_or_default(),
        context,
    )?;
    let number = |name: &str| -> Result<Brace> {
        let text = properties.child_text(name).unwrap_or("0").trim();
        text.parse::<i64>()
            .map(Brace::num)
            .map_err(|_| anyhow!("bad <{name}> {text:?}"))
    };
    Ok(row(brace_list![
        Brace::num(2),
        md_base(&object.uuid, properties),
        Brace::str(native_text(
            properties.child_text("Key").unwrap_or_default()
        )),
        Brace::str(native_text(
            properties.child_text("Description").unwrap_or_default()
        )),
        bool_prop(properties, "Use")?,
        bool_prop(properties, "Predefined")?,
        module,
        method,
        number("RestartCountOnFailure")?,
        number("RestartIntervalOnFailure")?,
    ]))
}

/// `{<collection uuid>,<count>,<item>...}`.
fn collection(uuid: &str, items: Vec<Brace>) -> Brace {
    let mut out = vec![Brace::uuid(uuid), Brace::num(items.len() as i64)];
    out.extend(items);
    Brace::List(out)
}

/// uuids of owned forms/templates named in `<ChildObjects>` (`<Form>F</Form>`).
fn owned_object_uuids(
    object: &ObjectXml<'_>,
    kind: &str,
    context: &DescriptorContext,
) -> Result<Vec<Brace>> {
    let mut out = Vec::new();
    if let Some(children) = object.child_objects() {
        for child in children.children_named(kind) {
            let full = format!(
                "{}.{}.{kind}.{}",
                object.kind,
                object.name,
                child.text.trim()
            );
            out.push(Brace::uuid(&object_uuid(&full, context)?));
        }
    }
    Ok(out)
}

const TEMPLATES: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
const SETTINGS_STORAGE_FORMS: &str = "b8533c0c-2342-4db3-91a2-c2b08cbf6b23";

/// Canonical collection classes already used by the SettingsStorage encoder.
pub(crate) fn owned_body_classes(kind: &str) -> Option<Vec<(&'static str, &'static str)>> {
    (kind == "SettingsStorage")
        .then(|| vec![(SETTINGS_STORAGE_FORMS, "Form"), (TEMPLATES, "Template")])
}

/// `{1,{2,{0,<md base>},<Manager ids>,<DefaultLoadForm>,<DefaultSaveForm>,
/// <AuxiliaryLoadForm>,<AuxiliarySaveForm>},2,{3daea016-...,<templates>},
/// {b8533c0c-...,<forms>}}`
fn settings_storage(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    let (manager_type, manager_value) = generated_ids(object.element, "Manager")?;
    let form = |name: &str| -> Result<Brace> {
        Ok(Brace::uuid(&form_uuid(
            properties.child_text(name).unwrap_or_default(),
            context,
        )?))
    };
    Ok(brace_list![
        Brace::num(1),
        brace_list![
            Brace::num(2),
            brace_list![Brace::num(0), md_base(&object.uuid, properties)],
            manager_type,
            manager_value,
            form("DefaultLoadForm")?,
            form("DefaultSaveForm")?,
            form("AuxiliaryLoadForm")?,
            form("AuxiliarySaveForm")?,
        ],
        Brace::num(2),
        collection(TEMPLATES, owned_object_uuids(object, "Template", context)?),
        collection(
            SETTINGS_STORAGE_FORMS,
            owned_object_uuids(object, "Form", context)?
        ),
    ])
}

/// `{1,{14,<Manager ids>,<List ids>,{2,<md base>,<type pattern>},
/// {0,<count>,<metadata ref>...},<UseStandardCommands>,<DefaultForm>,
/// <AuxiliaryForm>,<ListPresentation>,<ExtendedListPresentation>,
/// <Explanation>},2,{00867c40-...,<forms>},{23fa3b84-...,<commands>}}`
fn filter_criterion(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    const FORMS: &str = "00867c40-06b1-11d6-a3c7-0050bae0a776";
    const COMMANDS: &str = "23fa3b84-220a-40e9-8331-e588bed87f7d";
    let properties = object.properties()?;
    let (manager_type, manager_value) = generated_ids(object.element, "Manager")?;
    let (list_type, list_value) = generated_ids(object.element, "List")?;
    let mut content = vec![Brace::num(0), Brace::num(0)];
    if let Some(items) = properties.child("Content") {
        for item in items.children_named("Item") {
            content.push(metadata_ref(&item.text, context)?);
        }
    }
    content[1] = Brace::num((content.len() - 2) as i64);
    // Owned commands are the reference objects' `{{0,{0,0,0,<command>}},0}`.
    let commands = super::objects::parts::Obj::new(object, context)?
        .commands(super::objects::parts::CommandWrapper::Owner)?;
    Ok(brace_list![
        Brace::num(1),
        brace_list![
            Brace::num(14),
            manager_type,
            manager_value,
            list_type,
            list_value,
            brace_list![
                Brace::num(2),
                md_base(&object.uuid, properties),
                type_pattern(properties.child("Type"), context)?,
            ],
            Brace::List(content),
            bool_prop(properties, "UseStandardCommands")?,
            Brace::uuid(&form_uuid(
                properties.child_text("DefaultForm").unwrap_or_default(),
                context
            )?),
            Brace::uuid(&form_uuid(
                properties.child_text("AuxiliaryForm").unwrap_or_default(),
                context
            )?),
            localized_prop(properties, "ListPresentation"),
            localized_prop(properties, "ExtendedListPresentation"),
            localized_prop(properties, "Explanation"),
        ],
        Brace::num(2),
        collection(FORMS, owned_object_uuids(object, "Form", context)?),
        collection(COMMANDS, commands),
    ])
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::brace::serialize;
    use super::super::index::ObjectEntry;
    use super::super::types::tests::{context, element};
    use super::*;

    const MODULE: &str = "dc2b7a9e-132b-4a30-b7a8-ec72bf3d2e63";
    const JOB: &str = "c7ffd8ab-15e9-4cf1-a7fd-d05534dff000";

    fn compile_one(kind: &str, xml: &str) -> String {
        let mut context = context();
        context.index.objects.insert(
            "CommonModule.Курсы".to_string(),
            ObjectEntry {
                kind: "CommonModule".to_string(),
                name: "Курсы".to_string(),
                uuid: MODULE.to_string(),
                full_name: "CommonModule.Курсы".to_string(),
                path: PathBuf::new(),
            },
        );
        let element = element(xml);
        let object = ObjectXml {
            element: &element,
            kind,
            uuid: element.attr("uuid").unwrap_or_default().to_string(),
            name: element
                .path(&["Properties", "Name"])
                .map(|name| name.text.clone())
                .unwrap_or_default(),
            path: std::path::Path::new("."),
        };
        serialize(&compile(&object, &context).unwrap()).replace("\r\n", "")
    }

    #[test]
    fn a_scheduled_job_writes_key_before_description() {
        // BSP `ScheduledJobs/ЗагрузкаКурсовВалют` with a key and a description.
        let actual = compile_one(
            "ScheduledJob",
            &format!(
                r##"<ScheduledJob uuid="{JOB}"><Properties><Name>Загрузка</Name><Synonym/><Comment/><MethodName>CommonModule.Курсы.ПриЗагрузке</MethodName><Description>Описание</Description><Key>1</Key><Use>false</Use><Predefined>true</Predefined><RestartCountOnFailure>10</RestartCountOnFailure><RestartIntervalOnFailure>600</RestartIntervalOnFailure></Properties></ScheduledJob>"##
            ),
        );
        assert_eq!(
            actual,
            format!(
                r##"{{1,{{2,{{3,{{1,0,{JOB}}},"Загрузка",{{0}},"",0,0,{NIL_UUID},0}},"1","Описание",0,1,{MODULE},"ПриЗагрузке",10,600}},0}}"##
            )
        );
    }

    #[test]
    fn an_event_subscription_stores_the_bilingual_event_name() {
        let actual = compile_one(
            "EventSubscription",
            &format!(
                r##"<EventSubscription uuid="{JOB}"><Properties><Name>Подписка</Name><Synonym/><Comment/><Source><v8:Type>cfg:CatalogRef.Валюты</v8:Type></Source><Event>BeforeWrite</Event><Handler>CommonModule.Курсы.ПередЗаписью</Handler></Properties></EventSubscription>"##
            ),
        );
        let valuta = super::super::types::tests::VALUTA_TYPE;
        assert_eq!(
            actual,
            format!(
                r##"{{1,{{1,{{3,{{1,0,{JOB}}},"Подписка",{{0}},"",0,0,{NIL_UUID},0}},{{"Pattern",{{"#",{valuta}}}}},"BeforeWrite_ПередЗаписью",{MODULE},"ПередЗаписью"}},0}}"##
            )
        );
    }
}
