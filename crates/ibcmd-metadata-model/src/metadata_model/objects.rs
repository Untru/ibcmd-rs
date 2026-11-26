//! Reference objects: Catalog, Document, ExchangePlan, the three charts,
//! BusinessProcess, Task, Report, DataProcessor, Enum.
//!
//! Every kind is `{1,<owner record>,<n>,<collection>...}`: the owner record
//! is a flat list of slots (generated types, the md header, properties) and
//! each collection is `{<class uuid>,<count>,<item>...}`, the collections in
//! uuid order. A kind is a [`Layout`]: its slots named by the XML property
//! each carries, and its collections -- the model a row is compiled from and,
//! read the other way, decoded into. The parts every kind composes live in
//! `objects_parts.rs`: standard attributes, commands, tabular sections,
//! attribute wrappers, characteristics, field and metadata references.
//!
//! Record versions follow the configuration's compatibility mode, not the
//! platform: 8.3.24 stores the older versions; 8.3.27 adds
//! `LineNumberLength` to tabular sections, `TypeReductionMode` to standard
//! attributes and a constant tail to attributes; 8.5.1 adds
//! `AuxiliaryVariantForm` to reports and colours to enum values.

#[path = "objects_export.rs"]
pub(crate) mod export;
#[path = "objects_parts.rs"]
#[doc(hidden)]
pub mod parts;

use anyhow::{Result, anyhow};

use self::parts::{Obj, restore_crlf};
use super::brace::Brace;
use super::{DescriptorContext, ObjectXml};
pub(crate) use ibcmd_restructure::descriptor::*;

pub fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let layout =
        layout(object.kind).ok_or_else(|| anyhow!("{} is not a reference object", object.kind))?;
    let mut tree = Obj::new(object, context)?.compile(layout)?;
    restore_crlf(&mut tree);
    Ok(tree)
}

/// Body-owning collection identities from the existing typed layouts.
/// This reader does not alter encoding or admit a new dynamic body family.
#[doc(hidden)]
pub fn owned_body_classes(kind: &str) -> Option<Vec<(&'static str, &'static str)>> {
    if let Some(layout) = layout(kind) {
        return Some(
            layout
                .collections
                .iter()
                .filter_map(|(class, role)| match role {
                    Coll::Forms => Some((*class, "Form")),
                    Coll::Templates => Some((*class, "Template")),
                    _ => None,
                })
                .collect(),
        );
    }
    super::registers::owned_body_classes(kind).or_else(|| super::simple::owned_body_classes(kind))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::{DescriptorContext, compile_descriptor};

    const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">"#;

    /// An enum compiles to its stored row with no base: owner record,
    /// generated types, the four collections, the value records; the
    /// 8.5.1 compatibility adds the value colour.
    #[test]
    fn compiles_an_enum_without_a_base_row() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-objects-enum-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        fs::create_dir_all(root.join("Enums")).unwrap();
        let enum_xml = format!(
            r#"{HEAD}
<Enum uuid="0D2189F1-306C-4236-836A-1A4BFC0CFF13">
<InternalInfo>
<xr:GeneratedType name="EnumRef.E" category="Ref"><xr:TypeId>e4253022-3bfb-430b-bd60-e507622d28c9</xr:TypeId><xr:ValueId>2b90a755-c5f0-4075-b543-5eaf9c0f9b54</xr:ValueId></xr:GeneratedType>
<xr:GeneratedType name="EnumManager.E" category="Manager"><xr:TypeId>6a120ac5-26bf-4c49-b7e5-37917a0c787e</xr:TypeId><xr:ValueId>e8eb2b7a-fbde-45c9-9e8a-1a2a0a0832bf</xr:ValueId></xr:GeneratedType>
<xr:GeneratedType name="EnumList.E" category="List"><xr:TypeId>3a7ddce0-9165-4ff2-ab59-f91506e04201</xr:TypeId><xr:ValueId>6d76ebe3-a08e-4c5e-ad5d-c9f739008a98</xr:ValueId></xr:GeneratedType>
</InternalInfo>
<Properties><Name>E</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Два
строки</v8:content></v8:item></Synonym><Comment/>
<UseStandardCommands>false</UseStandardCommands><Characteristics/><QuickChoice>true</QuickChoice>
<ChoiceMode>BothWays</ChoiceMode><DefaultListForm/><DefaultChoiceForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/>
<ListPresentation/><ExtendedListPresentation/><Explanation/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput></Properties>
<ChildObjects><EnumValue uuid="a03d3535-090a-46f5-a411-3567c8c12bc1"><Properties><Name>V</Name><Synonym/><Comment/><Color>pal:Red</Color></Properties></EnumValue></ChildObjects>
</Enum></MetaDataObject>"#
        );
        let path = root.join("Enums").join("E.xml");
        fs::write(&path, &enum_xml).unwrap();
        let compile = |mode: &str| {
            fs::write(
                root.join("Configuration.xml"),
                format!(
                    "{HEAD}<Configuration uuid=\"11111111-1111-1111-1111-111111111111\"><Properties><Name>C</Name><CompatibilityMode>{mode}</CompatibilityMode></Properties></Configuration></MetaDataObject>"
                ),
            )
            .unwrap();
            let context = DescriptorContext::new(&root, "2.20").unwrap();
            let row = compile_descriptor("Enum", &path, enum_xml.as_bytes(), &context).unwrap();
            String::from_utf8(row).unwrap()
        };
        let row = compile("Version8_3_24");
        let nil = "00000000-0000-0000-0000-000000000000";
        let expected = format!(
            "\u{feff}{{1,{{20,e4253022-3bfb-430b-bd60-e507622d28c9,2b90a755-c5f0-4075-b543-5eaf9c0f9b54,\
             6a120ac5-26bf-4c49-b7e5-37917a0c787e,e8eb2b7a-fbde-45c9-9e8a-1a2a0a0832bf,\
             {{0,{{3,{{1,0,0d2189f1-306c-4236-836a-1a4bfc0cff13}},\"E\",{{1,\"ru\",\"Два\r\nстроки\"}},\"\",0,0,{nil},0}}}},\
             0,3a7ddce0-9165-4ff2-ab59-f91506e04201,6d76ebe3-a08e-4c5e-ad5d-c9f739008a98,{nil},{nil},2,1,{nil},{nil},\
             {{0}},{{0}},{{0}},{{0}},{{0,{{0}}}},0}},4,\
             {{33f2e54b-37ce-4a7a-a569-b648d7aa4634,0}},{{3daea016-69b7-4ed4-9453-127911372fe6,0}},\
             {{6d8d73a7-ba29-401d-9032-3872ec2d6433,0}},\
             {{bee0a08c-07eb-40c0-8544-5c364c171465,1,{{{{0,{{3,{{1,0,a03d3535-090a-46f5-a411-3567c8c12bc1}},\"V\",{{0}},\"\",0,0,{nil},0}}}},0}}}}}}"
        );
        // Layout breaks aside (they follow the one serializer rule), the
        // row is the stored one.
        assert_eq!(strip_layout(&row), expected);
        // The 8.5.1 compatibility spells the value colour.
        let row = compile("Version8_5_1");
        assert!(row.contains("{1,\r\n{3,\r\n{1,0,a03d3535-090a-46f5-a411-3567c8c12bc1}"));
        assert!(strip_layout(&row).contains(&format!("{nil},0}},{{4,4,{{2}},5}}}},0}}")));
    }

    /// Drops the CRLF the serializer puts before nested lists and closing
    /// braces, keeping CRLF inside strings.
    fn strip_layout(row: &str) -> String {
        let mut out = String::with_capacity(row.len());
        let mut in_string = false;
        let mut chars = row.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '"' {
                in_string = !in_string;
            }
            if !in_string && ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
                continue;
            }
            out.push(ch);
        }
        out
    }
}
