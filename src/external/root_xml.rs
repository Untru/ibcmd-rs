//! The root `<DataProcessor>`/`<Report>` XML the pipeline writes → the
//! platform's `<ExternalDataProcessor>`/`<ExternalReport>`: main uuid,
//! ContainedObject, no manager type, only the external property set.

use anyhow::{Result, bail};

use super::{header::ExternalMain, rename::to_external_references};

const DROPPED_PROPERTIES: [&str; 4] = [
    "UseStandardCommands",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
];

/// Removes a root-level property (`\t\t\t<tag…`: self-closed or not, with
/// attributes or not). A property still there afterwards -- a spelling this
/// does not follow -- is an error: printing a property the platform does not
/// write for an external object is not left to chance.
fn remove_root_property(xml: &str, tag: &str) -> Result<String> {
    let start = format!("\r\n\t\t\t<{tag}");
    let close = format!("</{tag}>");
    let mut out = xml.to_owned();
    while let Some(at) = out.find(&start) {
        let after = at + start.len();
        let Some(tag_end) = out[after..].find('>').map(|end| after + end) else {
            bail!("unterminated <{tag}> in the root XML");
        };
        let opened = &out[after..tag_end];
        if !(opened.is_empty() || opened == "/" || opened.starts_with(' ')) {
            // Another tag that only begins like this one.
            break;
        }
        let end = if opened.ends_with('/') {
            tag_end + 1
        } else {
            let Some(len) = out[tag_end..].find(&close) else {
                bail!("unterminated <{tag}> in the root XML");
            };
            tag_end + len + close.len()
        };
        out.replace_range(at..end, "");
    }
    if out.contains(&format!("{start}>"))
        || out.contains(&format!("{start}/>"))
        || out.contains(&format!("{start} "))
    {
        bail!("<{tag}> is still in the root XML");
    }
    Ok(out)
}

pub fn to_external_root(xml: &str, main: &ExternalMain) -> Result<String> {
    let internal = main.kind.internal_kind();
    let external = main.kind.external_kind();
    let open = format!("\t<{internal} uuid=\"{}\">", main.object_id);
    if !xml.contains(&open) {
        bail!("root XML does not open with `{open}`");
    }
    let mut out = xml.replacen(
        &open,
        &format!("\t<{external} uuid=\"{}\">", main.main_uuid),
        1,
    );
    out = out.replacen(&format!("\t</{internal}>"), &format!("\t</{external}>"), 1);
    if !out.contains("\t\t<InternalInfo>\r\n") {
        bail!("root XML has no InternalInfo");
    }
    let contained = format!(
        "\t\t<InternalInfo>\r\n\t\t\t<xr:ContainedObject>\r\n\t\t\t\t<xr:ClassId>{}</xr:ClassId>\r\n\t\t\t\t<xr:ObjectId>{}</xr:ObjectId>\r\n\t\t\t</xr:ContainedObject>\r\n",
        main.kind.class_id(),
        main.object_id
    );
    out = out.replacen("\t\t<InternalInfo>\r\n", &contained, 1);
    let manager = format!("\t\t\t<xr:GeneratedType name=\"{internal}Manager.");
    if let Some(at) = out.find(&manager) {
        const END: &str = "</xr:GeneratedType>\r\n";
        let Some(end) = out[at..].find(END).map(|e| at + e + END.len()) else {
            bail!("unterminated manager GeneratedType in root XML");
        };
        out.replace_range(at..end, "");
    }
    for tag in DROPPED_PROPERTIES {
        out = remove_root_property(&out, tag)?;
    }
    Ok(to_external_references(&out, main.kind, &main.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::{ExternalKind, header::ExternalMain};

    fn main_dp() -> ExternalMain {
        ExternalMain {
            kind: ExternalKind::DataProcessor,
            main_uuid: "d9704b20-c29c-4e7f-b777-0ac9080a5631".into(),
            object_id: "3b58e713-b1af-4db7-84e7-e4e30aff1c21".into(),
            name: "Инфо".into(),
            header: vec![],
            collections: vec![],
        }
    }

    const INTERNAL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject version=\"2.20\">\r\n\t<DataProcessor uuid=\"3b58e713-b1af-4db7-84e7-e4e30aff1c21\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:GeneratedType name=\"DataProcessorObject.Инфо\" category=\"Object\">\r\n\t\t\t\t<xr:TypeId>1</xr:TypeId>\r\n\t\t\t\t<xr:ValueId>2</xr:ValueId>\r\n\t\t\t</xr:GeneratedType>\r\n\t\t\t<xr:GeneratedType name=\"DataProcessorManager.Инфо\" category=\"Manager\">\r\n\t\t\t\t<xr:TypeId>3</xr:TypeId>\r\n\t\t\t\t<xr:ValueId>4</xr:ValueId>\r\n\t\t\t</xr:GeneratedType>\r\n\t\t</InternalInfo>\r\n\t\t<Properties>\r\n\t\t\t<Name>Инфо</Name>\r\n\t\t\t<Comment/>\r\n\t\t\t<UseStandardCommands>true</UseStandardCommands>\r\n\t\t\t<DefaultForm>DataProcessor.Инфо.Form.Форма</DefaultForm>\r\n\t\t\t<AuxiliaryForm/>\r\n\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n\t\t\t<ExtendedPresentation>\r\n\t\t\t\t<v8:item/>\r\n\t\t\t</ExtendedPresentation>\r\n\t\t\t<Explanation/>\r\n\t\t</Properties>\r\n\t\t<ChildObjects/>\r\n\t</DataProcessor>\r\n</MetaDataObject>";

    #[test]
    fn converts_internal_root_into_external_root() {
        let xml = to_external_root(INTERNAL, &main_dp()).unwrap();
        assert!(xml.contains("\t<ExternalDataProcessor uuid=\"d9704b20-c29c-4e7f-b777-0ac9080a5631\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:ContainedObject>\r\n\t\t\t\t<xr:ClassId>c3831ec8-d8d5-4f93-8a22-f9bfae07327f</xr:ClassId>\r\n\t\t\t\t<xr:ObjectId>3b58e713-b1af-4db7-84e7-e4e30aff1c21</xr:ObjectId>\r\n\t\t\t</xr:ContainedObject>\r\n\t\t\t<xr:GeneratedType name=\"ExternalDataProcessorObject.Инфо\""));
        assert!(!xml.contains("Manager"));
        for gone in [
            "UseStandardCommands",
            "IncludeHelpInContents",
            "ExtendedPresentation",
            "Explanation",
        ] {
            assert!(!xml.contains(gone), "{gone} left");
        }
        assert!(xml.contains("<DefaultForm>ExternalDataProcessor.Инфо.Form.Форма</DefaultForm>"));
        assert!(xml.ends_with("\t</ExternalDataProcessor>\r\n</MetaDataObject>"));
    }

    #[test]
    fn unexpected_root_fails_closed() {
        let other = INTERNAL.replace(
            "3b58e713-b1af-4db7-84e7-e4e30aff1c21\">",
            "ffffffff-b1af-4db7-84e7-e4e30aff1c21\">",
        );
        assert!(to_external_root(&other, &main_dp()).is_err());
    }

    #[test]
    fn a_dropped_property_goes_in_any_spelling_or_the_root_fails() {
        let xml = "\t\t<Properties>\r\n\t\t\t<Name>X</Name>\r\n\t\t\t<Explanation xsi:type=\"t\">\r\n\t\t\t\t<v8:item/>\r\n\t\t\t</Explanation>\r\n\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n\t\t</Properties>";
        let without = remove_root_property(xml, "Explanation").unwrap();
        assert_eq!(
            remove_root_property(&without, "IncludeHelpInContents").unwrap(),
            "\t\t<Properties>\r\n\t\t\t<Name>X</Name>\r\n\t\t</Properties>"
        );
        assert!(
            remove_root_property("\r\n\t\t\t<Explanation>unterminated", "Explanation").is_err()
        );
    }
}
