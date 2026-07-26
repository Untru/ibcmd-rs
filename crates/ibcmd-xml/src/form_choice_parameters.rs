//! Verified XML emission for Form `InputField.choiceParameters`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_schema::{
    FormChoiceParameterLink, FormChoiceParameterValue, FormChoiceParameterValuePart,
    FormChoiceParameters, FormChoiceParametersEmptyCollection, SchemaError, WriterPolicy,
    WriterRuleKey, WriterRuleLookupError, bundled_writer_rules,
    canonical_form_choice_parameters_qname, form_choice_parameter_cluster_order,
};

const MAX_INDENT: usize = 64;
const MAX_VALUE_BYTES: usize = 32 * 1024;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceParametersEmitError {
    Schema(SchemaError),
    WriterRule(WriterRuleLookupError),
    InvalidPolicy,
    InvalidValue(&'static str),
    LimitExceeded(&'static str),
}

impl Display for FormChoiceParametersEmitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(error) => write!(formatter, "{error}"),
            Self::WriterRule(error) => write!(formatter, "{error}"),
            Self::InvalidPolicy => {
                write!(formatter, "invalid verified Form choice-parameters policy")
            }
            Self::InvalidValue(field) => {
                write!(
                    formatter,
                    "invalid Form choice-parameters value for {field}"
                )
            }
            Self::LimitExceeded(field) => {
                write!(
                    formatter,
                    "Form choice-parameters limit exceeded for {field}"
                )
            }
        }
    }
}

impl Error for FormChoiceParametersEmitError {}

impl From<SchemaError> for FormChoiceParametersEmitError {
    fn from(error: SchemaError) -> Self {
        Self::Schema(error)
    }
}

impl From<WriterRuleLookupError> for FormChoiceParametersEmitError {
    fn from(error: WriterRuleLookupError) -> Self {
        Self::WriterRule(error)
    }
}

/// Emits the verified `ChoiceParameters` owner fragment. Empty canonical values
/// intentionally produce no XML because the bundled policy omits the wrapper.
pub fn emit_form_choice_parameters(
    parameters: &FormChoiceParameters,
    indent: usize,
) -> Result<String, FormChoiceParametersEmitError> {
    let policy = resolved_policy()?;
    preflight(parameters, indent, &policy)?;
    if parameters.is_empty() {
        return Ok(String::new());
    }

    let mut counter = CountingSink::default();
    emit_into(&mut counter, parameters, indent, &policy)?;
    let exact_len = counter.len;
    let mut output = String::with_capacity(exact_len);
    emit_into(&mut output, parameters, indent, &policy)?;
    debug_assert_eq!(output.len(), exact_len);
    Ok(output)
}

/// Emits the verified `ChoiceParameterLinks` predecessor fragment from typed
/// links. The wrapper name and its position are resolved from the same exact
/// EDT policy as `ChoiceParameters`.
pub fn emit_form_choice_parameter_links(
    links: &[FormChoiceParameterLink],
    indent: usize,
) -> Result<String, FormChoiceParametersEmitError> {
    if links.is_empty() {
        return Ok(String::new());
    }
    if indent > MAX_INDENT {
        return Err(FormChoiceParametersEmitError::LimitExceeded("indent"));
    }
    for link in links {
        validate_value("link name", link.name())?;
        validate_value("link data path", link.data_path())?;
    }
    let policy = choice_parameters_policy()?;
    let wrapper = form_choice_parameter_cluster_order(&policy)?[0].xml_local_name();

    let mut counter = CountingSink::default();
    emit_links_into(&mut counter, links, indent, wrapper)?;
    let mut output = String::with_capacity(counter.len);
    emit_links_into(&mut output, links, indent, wrapper)?;
    debug_assert_eq!(output.len(), counter.len);
    Ok(output)
}

fn choice_parameters_policy() -> Result<WriterPolicy, FormChoiceParametersEmitError> {
    let corpus = bundled_writer_rules()?;
    let rule = corpus.exact_rule(WriterRuleKey {
        source_release: "2025.2.3+30",
        model_type: "InputFieldExtInfo",
        feature: "choiceParameters",
    })?;
    rule.policy
        .clone()
        .ok_or(FormChoiceParametersEmitError::InvalidPolicy)
}

struct ResolvedPolicy {
    owner: String,
    item: String,
    name_attribute: String,
    value: String,
    presentation: String,
    scalar_value: String,
    fixed_array_item: String,
    value_xsi_type: String,
    boolean_xsi_type: String,
    design_time_ref_xsi_type: String,
    value_order: Vec<FormChoiceParameterValuePart>,
    fixed_array_xsi_type: String,
    fixed_array_item_xsi_type: String,
    fixed_array_item_order: Vec<FormChoiceParameterValuePart>,
}

fn resolved_policy() -> Result<ResolvedPolicy, FormChoiceParametersEmitError> {
    let policy = choice_parameters_policy()?;
    let Some(WriterPolicy::FormChoiceParameters {
        owner_qname,
        empty_collection: FormChoiceParametersEmptyCollection::OmitWhenWriteDefaultFalse,
        item,
        fixed_array,
        ..
    }) = Some(&policy)
    else {
        return Err(FormChoiceParametersEmitError::InvalidPolicy);
    };

    Ok(ResolvedPolicy {
        owner: canonical_form_choice_parameters_qname(owner_qname)?,
        item: canonical_form_choice_parameters_qname(&item.item_qname)?,
        name_attribute: canonical_form_choice_parameters_qname(&item.name_attribute_qname)?,
        value: canonical_form_choice_parameters_qname(&item.value_qname)?,
        presentation: canonical_form_choice_parameters_qname(&item.presentation_qname)?,
        scalar_value: canonical_form_choice_parameters_qname(&item.scalar_value_qname)?,
        fixed_array_item: canonical_form_choice_parameters_qname(&fixed_array.item_qname)?,
        value_xsi_type: item.value_xsi_type.clone(),
        boolean_xsi_type: item.boolean_xsi_type.clone(),
        design_time_ref_xsi_type: item.design_time_ref_xsi_type.clone(),
        value_order: item.value_order.clone(),
        fixed_array_xsi_type: fixed_array.xsi_type.clone(),
        fixed_array_item_xsi_type: fixed_array.item_xsi_type.clone(),
        fixed_array_item_order: fixed_array.item_order.clone(),
    })
}

fn emit_links_into(
    sink: &mut impl Sink,
    links: &[FormChoiceParameterLink],
    indent: usize,
    wrapper: &str,
) -> Result<(), FormChoiceParametersEmitError> {
    push_indent(sink, indent, 0)?;
    sink.push("<")?;
    sink.push(wrapper)?;
    sink.push(">\r\n")?;
    for link in links {
        push_indent(sink, indent, 1)?;
        sink.push("<xr:Link>\r\n")?;
        push_indent(sink, indent, 2)?;
        sink.push("<xr:Name>")?;
        push_escaped(sink, link.name(), EscapeMode::Text)?;
        sink.push("</xr:Name>\r\n")?;
        push_indent(sink, indent, 2)?;
        sink.push("<xr:DataPath xsi:type=\"xs:string\">")?;
        push_escaped(sink, link.data_path(), EscapeMode::Text)?;
        sink.push("</xr:DataPath>\r\n")?;
        push_indent(sink, indent, 2)?;
        sink.push("<xr:ValueChange>")?;
        sink.push(link.value_change().xml_value())?;
        sink.push("</xr:ValueChange>\r\n")?;
        push_indent(sink, indent, 1)?;
        sink.push("</xr:Link>\r\n")?;
    }
    push_indent(sink, indent, 0)?;
    sink.push("</")?;
    sink.push(wrapper)?;
    sink.push(">\r\n")
}

fn preflight(
    parameters: &FormChoiceParameters,
    indent: usize,
    policy: &ResolvedPolicy,
) -> Result<(), FormChoiceParametersEmitError> {
    if indent > MAX_INDENT {
        return Err(FormChoiceParametersEmitError::LimitExceeded("indent"));
    }
    for value in [
        &policy.value_xsi_type,
        &policy.boolean_xsi_type,
        &policy.design_time_ref_xsi_type,
        &policy.fixed_array_xsi_type,
        &policy.fixed_array_item_xsi_type,
    ] {
        validate_value("xsi:type", value)?;
    }
    for parameter in parameters.items() {
        validate_value("name", parameter.name())?;
        validate_presentation(parameter.presentation())?;
        match parameter.value() {
            FormChoiceParameterValue::Boolean(_) => {}
            FormChoiceParameterValue::DesignTimeRef(reference) => {
                validate_value("design-time reference", reference)?;
            }
            FormChoiceParameterValue::FixedArray(values) => {
                for value in values {
                    validate_presentation(value.presentation())?;
                    validate_value("design-time reference", value.value_ref())?;
                }
            }
        }
    }
    Ok(())
}

fn validate_presentation(values: &[(String, String)]) -> Result<(), FormChoiceParametersEmitError> {
    for (language, content) in values {
        validate_value("presentation language", language)?;
        validate_value("presentation content", content)?;
    }
    Ok(())
}

fn validate_value(field: &'static str, value: &str) -> Result<(), FormChoiceParametersEmitError> {
    if value.len() > MAX_VALUE_BYTES {
        Err(FormChoiceParametersEmitError::LimitExceeded(field))
    } else if value.chars().any(|character| !is_xml_1_0(character)) {
        Err(FormChoiceParametersEmitError::InvalidValue(field))
    } else {
        Ok(())
    }
}

trait Sink {
    fn push(&mut self, value: &str) -> Result<(), FormChoiceParametersEmitError>;
}

#[derive(Default)]
struct CountingSink {
    len: usize,
}

impl Sink for CountingSink {
    fn push(&mut self, value: &str) -> Result<(), FormChoiceParametersEmitError> {
        self.len = self
            .len
            .checked_add(value.len())
            .filter(|length| *length <= MAX_OUTPUT_BYTES)
            .ok_or(FormChoiceParametersEmitError::LimitExceeded("output"))?;
        Ok(())
    }
}

impl Sink for String {
    fn push(&mut self, value: &str) -> Result<(), FormChoiceParametersEmitError> {
        self.push_str(value);
        Ok(())
    }
}

fn emit_into(
    sink: &mut impl Sink,
    parameters: &FormChoiceParameters,
    indent: usize,
    policy: &ResolvedPolicy,
) -> Result<(), FormChoiceParametersEmitError> {
    push_indent(sink, indent, 0)?;
    sink.push("<")?;
    sink.push(&policy.owner)?;
    sink.push(">\r\n")?;
    for parameter in parameters.items() {
        push_indent(sink, indent, 1)?;
        sink.push("<")?;
        sink.push(&policy.item)?;
        sink.push(" ")?;
        sink.push(&policy.name_attribute)?;
        sink.push("=\"")?;
        push_escaped(sink, parameter.name(), EscapeMode::Attribute)?;
        sink.push("\">\r\n")?;
        push_indent(sink, indent, 2)?;
        sink.push("<")?;
        sink.push(&policy.value)?;
        sink.push(" xsi:type=\"")?;
        push_escaped(sink, &policy.value_xsi_type, EscapeMode::Attribute)?;
        sink.push("\">\r\n")?;
        for part in &policy.value_order {
            match part {
                FormChoiceParameterValuePart::Presentation => {
                    emit_presentation(sink, parameter.presentation(), indent + 3, policy)?;
                }
                FormChoiceParameterValuePart::Value => {
                    emit_value(sink, parameter.value(), indent, policy)?;
                }
            }
        }
        push_indent(sink, indent, 2)?;
        sink.push("</")?;
        sink.push(&policy.value)?;
        sink.push(">\r\n")?;
        push_indent(sink, indent, 1)?;
        sink.push("</")?;
        sink.push(&policy.item)?;
        sink.push(">\r\n")?;
    }
    push_indent(sink, indent, 0)?;
    sink.push("</")?;
    sink.push(&policy.owner)?;
    sink.push(">\r\n")
}

fn emit_value(
    sink: &mut impl Sink,
    value: &FormChoiceParameterValue,
    indent: usize,
    policy: &ResolvedPolicy,
) -> Result<(), FormChoiceParametersEmitError> {
    push_indent(sink, indent, 3)?;
    sink.push("<")?;
    sink.push(&policy.scalar_value)?;
    sink.push(" xsi:type=\"")?;
    match value {
        FormChoiceParameterValue::Boolean(boolean) => {
            push_escaped(sink, &policy.boolean_xsi_type, EscapeMode::Attribute)?;
            sink.push("\">")?;
            sink.push(if *boolean { "true" } else { "false" })?;
            sink.push("</")?;
            sink.push(&policy.scalar_value)?;
            sink.push(">\r\n")
        }
        FormChoiceParameterValue::DesignTimeRef(reference) => {
            push_escaped(
                sink,
                &policy.design_time_ref_xsi_type,
                EscapeMode::Attribute,
            )?;
            sink.push("\">")?;
            push_escaped(sink, reference, EscapeMode::Text)?;
            sink.push("</")?;
            sink.push(&policy.scalar_value)?;
            sink.push(">\r\n")
        }
        FormChoiceParameterValue::FixedArray(values) if values.is_empty() => {
            push_escaped(sink, &policy.fixed_array_xsi_type, EscapeMode::Attribute)?;
            sink.push("\"/>\r\n")
        }
        FormChoiceParameterValue::FixedArray(values) => {
            push_escaped(sink, &policy.fixed_array_xsi_type, EscapeMode::Attribute)?;
            sink.push("\">\r\n")?;
            for entry in values {
                push_indent(sink, indent, 4)?;
                sink.push("<")?;
                sink.push(&policy.fixed_array_item)?;
                sink.push(" xsi:type=\"")?;
                push_escaped(
                    sink,
                    &policy.fixed_array_item_xsi_type,
                    EscapeMode::Attribute,
                )?;
                sink.push("\">\r\n")?;
                for part in &policy.fixed_array_item_order {
                    match part {
                        FormChoiceParameterValuePart::Presentation => {
                            emit_presentation(sink, entry.presentation(), indent + 5, policy)?;
                        }
                        FormChoiceParameterValuePart::Value => {
                            push_indent(sink, indent, 5)?;
                            sink.push("<")?;
                            sink.push(&policy.scalar_value)?;
                            sink.push(" xsi:type=\"")?;
                            push_escaped(
                                sink,
                                &policy.design_time_ref_xsi_type,
                                EscapeMode::Attribute,
                            )?;
                            sink.push("\">")?;
                            push_escaped(sink, entry.value_ref(), EscapeMode::Text)?;
                            sink.push("</")?;
                            sink.push(&policy.scalar_value)?;
                            sink.push(">\r\n")?;
                        }
                    }
                }
                push_indent(sink, indent, 4)?;
                sink.push("</")?;
                sink.push(&policy.fixed_array_item)?;
                sink.push(">\r\n")?;
            }
            push_indent(sink, indent, 3)?;
            sink.push("</")?;
            sink.push(&policy.scalar_value)?;
            sink.push(">\r\n")
        }
    }
}

fn emit_presentation(
    sink: &mut impl Sink,
    values: &[(String, String)],
    indent: usize,
    policy: &ResolvedPolicy,
) -> Result<(), FormChoiceParametersEmitError> {
    push_indent(sink, indent, 0)?;
    sink.push("<")?;
    sink.push(&policy.presentation)?;
    if values.is_empty() {
        return sink.push("/>\r\n");
    }
    sink.push(">\r\n")?;
    for (language, content) in values {
        push_indent(sink, indent, 1)?;
        sink.push("<v8:item>\r\n")?;
        push_indent(sink, indent, 2)?;
        sink.push("<v8:lang>")?;
        push_escaped(sink, language, EscapeMode::Presentation)?;
        sink.push("</v8:lang>\r\n")?;
        push_indent(sink, indent, 2)?;
        sink.push("<v8:content>")?;
        push_escaped(sink, content, EscapeMode::Presentation)?;
        sink.push("</v8:content>\r\n")?;
        push_indent(sink, indent, 1)?;
        sink.push("</v8:item>\r\n")?;
    }
    push_indent(sink, indent, 0)?;
    sink.push("</")?;
    sink.push(&policy.presentation)?;
    sink.push(">\r\n")
}

fn push_indent(
    sink: &mut impl Sink,
    base: usize,
    extra: usize,
) -> Result<(), FormChoiceParametersEmitError> {
    for _ in 0..base
        .checked_add(extra)
        .ok_or(FormChoiceParametersEmitError::LimitExceeded("indent"))?
    {
        sink.push("\t")?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum EscapeMode {
    Attribute,
    Text,
    Presentation,
}

fn push_escaped(
    sink: &mut impl Sink,
    value: &str,
    mode: EscapeMode,
) -> Result<(), FormChoiceParametersEmitError> {
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if matches!(mode, EscapeMode::Presentation)
            && character == '\r'
            && characters.peek() == Some(&'\n')
        {
            characters.next();
            sink.push("\n")?;
            continue;
        }
        match character {
            '&' => sink.push("&amp;")?,
            '<' => sink.push("&lt;")?,
            '>' => sink.push("&gt;")?,
            '"' if matches!(mode, EscapeMode::Attribute) => sink.push("&quot;")?,
            _ => {
                let mut bytes = [0; 4];
                sink.push(character.encode_utf8(&mut bytes))?;
            }
        }
    }
    Ok(())
}

fn is_xml_1_0(character: char) -> bool {
    matches!(
        character,
        '\u{9}'
            | '\u{a}'
            | '\u{d}'
            | '\u{20}'..='\u{d7ff}'
            | '\u{e000}'..='\u{fffd}'
            | '\u{10000}'..='\u{10ffff}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibcmd_schema::{FormChoiceParameterLinkValueChange, parse_form_choice_parameters};

    const DISCRIMINATOR: &str = "0e704aa2-07bd-48b9-8223-a0212c4d5fc2";
    const ARRAY_TYPE: &str = "4500381b-db30-4a10-9db4-990038032acf";
    const NIL: &str = "00000000-0000-0000-0000-000000000000";
    const TYPE_ID: &str = "11111111-1111-4111-8111-111111111111";
    const VALUE_ID: &str = "22222222-2222-4222-8222-222222222222";

    fn quoted(value: &str) -> String {
        format!("\"{}\"", value.replace('"', "\"\""))
    }

    fn presentation(values: &[(&str, &str)]) -> String {
        let entries = values
            .iter()
            .map(|(language, content)| format!("{{{},{}}}", quoted(language), quoted(content)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{1,{}{}}}",
            values.len(),
            if entries.is_empty() {
                String::new()
            } else {
                format!(",{entries}")
            }
        )
    }

    fn parameter_with_reference(name: &str, value: &str, reference: &str) -> FormChoiceParameters {
        let raw = format!("{{0,1,{},{value}}}", quoted(name));
        parse_form_choice_parameters(&raw, |_, _| Some(reference.to_owned())).unwrap()
    }

    fn parameter(name: &str, value: &str) -> FormChoiceParameters {
        parameter_with_reference(name, value, "Enum.Kind.EnumValue.Value")
    }

    fn boolean(name: &str, presentation: &[(&str, &str)]) -> FormChoiceParameters {
        parameter(
            name,
            &format!(
                "{{\"#\",{DISCRIMINATOR},{{0,1,{{\"B\",1}},{NIL},{NIL},{}}}}}",
                self::presentation(presentation)
            ),
        )
    }

    fn design_ref(presentation: &[(&str, &str)], reference: &str) -> FormChoiceParameters {
        parameter_with_reference(
            "Ref",
            &format!(
                "{{\"#\",{DISCRIMINATOR},{{0,0,{{\"U\"}},{TYPE_ID},{VALUE_ID},{}}}}}",
                self::presentation(presentation)
            ),
            reference,
        )
    }

    fn fixed_array(entries: &[(&str, &[(&str, &str)])]) -> FormChoiceParameters {
        let count = entries.len();
        let serialized_entries = entries
            .iter()
            .map(|(value_id, presentation)| {
                format!(
                    "{{\"#\",{DISCRIMINATOR},{{0,0,{{\"U\"}},{TYPE_ID},{value_id},{}}}}}",
                    self::presentation(presentation)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let values = if entries.is_empty() {
            "0".to_owned()
        } else {
            format!("{count},{serialized_entries}")
        };
        parameter(
            "Array",
            &format!(
                "{{\"#\",{DISCRIMINATOR},{{0,1,{{\"#\",{ARRAY_TYPE},{{{values}}}}},{NIL},{NIL},{{1,0}}}}}}"
            ),
        )
    }

    #[test]
    fn empty_model_is_omitted() {
        let parameters = parse_form_choice_parameters("{0,0}", |_, _| None).unwrap();
        assert_eq!(emit_form_choice_parameters(&parameters, 0).unwrap(), "");
    }

    #[test]
    fn typed_links_use_exact_schema_lexicals_and_order() {
        let links = [
            FormChoiceParameterLink::new(
                "Owner".to_owned(),
                "Object.Ref".to_owned(),
                FormChoiceParameterLinkValueChange::Clear,
            ),
            FormChoiceParameterLink::new(
                "Keep".to_owned(),
                "Object.Kind".to_owned(),
                FormChoiceParameterLinkValueChange::DontChange,
            ),
        ];
        assert_eq!(
            emit_form_choice_parameter_links(&links, 1).unwrap(),
            "\t<ChoiceParameterLinks>\r\n\
\t\t<xr:Link>\r\n\
\t\t\t<xr:Name>Owner</xr:Name>\r\n\
\t\t\t<xr:DataPath xsi:type=\"xs:string\">Object.Ref</xr:DataPath>\r\n\
\t\t\t<xr:ValueChange>Clear</xr:ValueChange>\r\n\
\t\t</xr:Link>\r\n\
\t\t<xr:Link>\r\n\
\t\t\t<xr:Name>Keep</xr:Name>\r\n\
\t\t\t<xr:DataPath xsi:type=\"xs:string\">Object.Kind</xr:DataPath>\r\n\
\t\t\t<xr:ValueChange>DontChange</xr:ValueChange>\r\n\
\t\t</xr:Link>\r\n\
\t</ChoiceParameterLinks>\r\n"
        );
    }

    #[test]
    fn boolean_has_exact_crlf_tabs_and_attribute_escaping() {
        let parameters = boolean("a'\"<&>", &[]);
        let xml = emit_form_choice_parameters(&parameters, 1).unwrap();
        assert!(xml.contains("<app:item name=\"a'&quot;&lt;&amp;&gt;\">"));
        assert!(xml.contains("<Value xsi:type=\"xs:boolean\">true</Value>\r\n"));
        assert!(!xml.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn design_ref_preserves_crlf_and_escapes_only_text_markup() {
        let parameters = design_ref(&[], "line1\r\nline2<&>\"'");
        let xml = emit_form_choice_parameters(&parameters, 0).unwrap();
        assert!(xml.contains(
            "<Value xsi:type=\"xr:DesignTimeRef\">line1\r\nline2&lt;&amp;&gt;\"'</Value>"
        ));
        assert_eq!(parameters.items().len(), 1);
    }

    #[test]
    fn fixed_array_empty_and_nonempty_keep_verified_order() {
        let empty = emit_form_choice_parameters(&fixed_array(&[]), 0).unwrap();
        assert!(empty.contains("<Value xsi:type=\"v8:FixedArray\"/>"));
        let nonempty =
            emit_form_choice_parameters(&fixed_array(&[(VALUE_ID, &[("ru", "Первый")])]), 0)
                .unwrap();
        let presentation = nonempty.find("<Presentation>").unwrap();
        let value = nonempty
            .find("<Value xsi:type=\"xr:DesignTimeRef\">")
            .unwrap();
        assert!(presentation < value);
        assert!(nonempty.contains("<v8:Value xsi:type=\"FormChoiceListDesTimeValue\">"));
    }

    #[test]
    fn localized_presentation_normalizes_crlf_and_preserves_quotes_and_apostrophe() {
        let xml = emit_form_choice_parameters(
            &boolean(
                "Name",
                &[("en", "line1\r\nline2 \"quoted\" 'apostrophe' &<>")],
            ),
            0,
        )
        .unwrap();
        assert!(xml.contains(
            "<v8:content>line1\nline2 \"quoted\" 'apostrophe' &amp;&lt;&gt;</v8:content>\r\n"
        ));
        assert!(!xml.contains("line1\r\nline2"));
    }

    #[test]
    fn rejects_invalid_xml_control_and_indent_or_value_limits() {
        assert_eq!(
            emit_form_choice_parameters(&boolean("bad\u{1}", &[]), 0),
            Err(FormChoiceParametersEmitError::InvalidValue("name"))
        );
        assert_eq!(
            emit_form_choice_parameters(&boolean("ok", &[]), MAX_INDENT + 1),
            Err(FormChoiceParametersEmitError::LimitExceeded("indent"))
        );
        let oversized = "x".repeat(MAX_VALUE_BYTES + 1);
        assert_eq!(
            emit_form_choice_parameters(&boolean(&oversized, &[]), 0),
            Err(FormChoiceParametersEmitError::LimitExceeded("name"))
        );
    }

    #[test]
    fn exact_counter_accepts_near_output_limit_without_overallocating() {
        let chunk = "&".repeat(25 * 1024);
        let xml = emit_form_choice_parameters(
            &boolean("near-limit", &[("en", &chunk), ("ru", &chunk)]),
            0,
        )
        .unwrap();
        assert!(xml.len() > 250 * 1024);
        assert!(xml.len() < MAX_OUTPUT_BYTES);
        assert_eq!(xml.capacity(), xml.len());

        let over_limit = "&".repeat(27 * 1024);
        assert_eq!(
            emit_form_choice_parameters(
                &boolean("over-limit", &[("en", &over_limit), ("ru", &over_limit)]),
                0
            ),
            Err(FormChoiceParametersEmitError::LimitExceeded("output"))
        );
    }
}
