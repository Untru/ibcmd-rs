//! One measured WebSocketClient model shared by XML load and row export.

use anyhow::{Context, Result, ensure};
use ibcmd_schema::websocket_client::WebSocketClientLayout as Layout;

use super::brace::Brace;
use super::common::Header;
use super::export::values::{bool_text, header, header_elements};
use super::export::{Build, ObjectNames, atom, el, item, leaf, list, number, string, xml_text};
use super::xml::Element;
use super::{ObjectXml, native_text, parse_bool};
use crate::brace_list;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSocketClient {
    pub header: Header,
    pub predefined: bool,
    pub auto_connect: bool,
    pub server_url: String,
    pub user: String,
    pub password: String,
    pub headers: Vec<(String, String)>,
    pub use_os_proxy: bool,
    pub use_os_authentication: bool,
    pub timeout: i64,
}

impl WebSocketClient {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        ensure!(
            object.element.attrs.len() == 1 && object.element.attr("uuid").is_some(),
            "WebSocketClient requires only its UUID attribute"
        );
        uuid::Uuid::parse_str(&object.uuid).context("WebSocketClient invalid UUID")?;
        let properties = object.properties()?;
        ensure!(
            properties.attrs.is_empty()
                && properties.text.trim().is_empty()
                && object.element.text.trim().is_empty(),
            "WebSocketClient unsupported object or Properties text/attributes"
        );
        ensure!(
            object.element.children_named("Properties").count() == 1,
            "WebSocketClient duplicate Properties"
        );
        for child in &properties.children {
            ensure!(
                matches!(
                    child.name.as_str(),
                    "Name"
                        | "Synonym"
                        | "Comment"
                        | "Predefined"
                        | "AutoConnect"
                        | "ServerURL"
                        | "User"
                        | "Password"
                        | "Headers"
                        | "UseOSProxy"
                        | "UseOSAuthentication"
                        | "Timeout"
                ),
                "WebSocketClient unsupported property {}",
                child.name
            );
            ensure!(
                properties.children_named(&child.name).count() == 1,
                "WebSocketClient duplicate property {}",
                child.name
            );
        }
        ensure!(
            object
                .element
                .children
                .iter()
                .all(|child| child.name == "Properties"
                    || (child.name == "ChildObjects"
                        && child.children.is_empty()
                        && child.attrs.is_empty()
                        && child.text.trim().is_empty())),
            "WebSocketClient unsupported child collection"
        );
        let scalar = |name| -> Result<&str> {
            let element = object.prop(name)?;
            ensure!(
                element.children.is_empty() && element.attrs.is_empty(),
                "WebSocketClient {name} must be a scalar"
            );
            Ok(&element.text)
        };
        let boolean = |name| {
            parse_bool(scalar(name)?.trim()).with_context(|| format!("WebSocketClient {name}"))
        };
        scalar("Name")?;
        scalar("Comment")?;
        let synonym = object.prop("Synonym")?;
        ensure!(
            synonym.attrs.is_empty() && synonym.text.trim().is_empty(),
            "WebSocketClient invalid Synonym"
        );
        for entry in &synonym.children {
            ensure!(
                entry.name == "item"
                    && entry.attrs.is_empty()
                    && entry.text.trim().is_empty()
                    && entry.children.len() == 2,
                "WebSocketClient invalid Synonym item"
            );
            for (field, name) in entry.children.iter().zip(["lang", "content"]) {
                ensure!(
                    field.name == name && field.attrs.is_empty() && field.children.is_empty(),
                    "WebSocketClient invalid Synonym {name}"
                );
            }
        }
        let headers = object.prop("Headers")?;
        ensure!(
            headers
                .attr("type")
                .is_none_or(|ty| ty.rsplit(':').next() == Some("ValueList")),
            "WebSocketClient Headers must be a ValueList"
        );
        ensure!(
            headers.attrs.len() == usize::from(headers.attr("type").is_some()),
            "WebSocketClient Headers unsupported attribute"
        );
        ensure!(
            headers.text.trim().is_empty(),
            "WebSocketClient Headers has direct text"
        );
        let mut pairs = Vec::new();
        for entry in &headers.children {
            ensure!(
                entry.name == "Item"
                    && entry.children.len() == 3
                    && entry.text.trim().is_empty()
                    && entry.attrs.is_empty(),
                "WebSocketClient invalid Headers item"
            );
            let presentation = &entry.children[0];
            let state = &entry.children[1];
            let value = &entry.children[2];
            ensure!(
                presentation.name == "Presentation"
                    && presentation.children.is_empty()
                    && presentation.text.is_empty()
                    && presentation.attrs.is_empty(),
                "WebSocketClient Headers presentation cannot be stored"
            );
            ensure!(
                state.name == "CheckState"
                    && state.children.is_empty()
                    && state.text.trim() == "0"
                    && state.attrs.is_empty(),
                "WebSocketClient Headers check state cannot be stored"
            );
            ensure!(
                value.name == "Value"
                    && value.attrs.len() == 1
                    && value.children.len() == 2
                    && value.text.trim().is_empty()
                    && value
                        .attr("type")
                        .is_some_and(|ty| ty.rsplit(':').next() == Some("KeyAndValue")),
                "WebSocketClient Headers must contain KeyAndValue"
            );
            let key = &value.children[0];
            let text = &value.children[1];
            for (element, name) in [(key, "Key"), (text, "Value")] {
                ensure!(
                    element.name == name
                        && element.children.is_empty()
                        && element.attrs.len() == 1
                        && element
                            .attr("type")
                            .is_some_and(|ty| ty.rsplit(':').next() == Some("string")),
                    "WebSocketClient Headers {name} must be a string"
                );
            }
            pairs.push((native_text(&key.text), native_text(&text.text)));
        }
        Ok(Self {
            header: Header::of(object)?,
            predefined: boolean("Predefined")?,
            auto_connect: boolean("AutoConnect")?,
            server_url: native_text(scalar("ServerURL")?),
            user: native_text(scalar("User")?),
            password: native_text(scalar("Password")?),
            headers: pairs,
            use_os_proxy: boolean("UseOSProxy")?,
            use_os_authentication: boolean("UseOSAuthentication")?,
            timeout: scalar("Timeout")?
                .trim()
                .parse()
                .context("WebSocketClient Timeout must be an integer")?,
        })
    }

    pub fn to_brace(&self) -> Brace {
        let mut headers = vec![Brace::atom(self.headers.len())];
        for (key, value) in &self.headers {
            headers.push(Brace::str(key));
            headers.push(Brace::str(value));
        }
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(0),
                self.header.to_brace(),
                Brace::flag(self.predefined),
                Brace::str(&self.server_url),
                Brace::str(&self.user),
                Brace::str(&self.password),
                Brace::list(headers),
                Brace::flag(self.use_os_proxy),
                Brace::flag(self.use_os_authentication),
                Brace::num(self.timeout),
                Brace::flag(self.auto_connect)
            ],
            Brace::num(0)
        ]
    }

    pub fn from_brace(row: &Brace) -> Result<Self> {
        (|| -> Result<Self> {
            let root = list(row)?;
            ensure!(
                root.len() == Layout::OUTER_ARITY
                    && atom(item(root, 0)?)? == "1"
                    && atom(item(root, 2)?)? == "0",
                "invalid descriptor envelope"
            );
            let fields = list(&root[1])?;
            ensure!(
                fields.len() == Layout::PAYLOAD_ARITY && atom(&fields[0])? == "0",
                "invalid descriptor payload"
            );
            let raw_head = list(&fields[Layout::HEADER])?;
            let head = header(&fields[Layout::HEADER])?;
            let identity = list(&raw_head[1])?;
            ensure!(
                identity.len() == 3 && atom(&identity[0])? == "1" && atom(&identity[1])? == "0",
                "invalid header identity"
            );
            uuid::Uuid::parse_str(&head.uuid).context("invalid header UUID")?;
            ensure!(
                [5, 6, 8]
                    .iter()
                    .all(|index| raw_head[*index].as_atom() == Some("0"))
                    && uuid::Uuid::parse_str(atom(&raw_head[7])?)?.is_nil(),
                "unsupported header fields"
            );
            let localized = list(&head.synonym)?;
            let count: usize = atom(item(localized, 0)?)?
                .parse()
                .context("invalid synonym count")?;
            ensure!(
                Layout::headers_arity(count) == Some(localized.len()),
                "invalid synonym arity"
            );
            let mut synonym = Vec::new();
            for pair in localized[1..].chunks_exact(2) {
                synonym.push((string(&pair[0])?.to_owned(), string(&pair[1])?.to_owned()));
            }
            let headers = list(&fields[Layout::HEADERS])?;
            let count: usize = atom(item(headers, 0)?)?
                .parse()
                .context("invalid Headers count")?;
            ensure!(
                Layout::headers_arity(count) == Some(headers.len()),
                "invalid Headers arity"
            );
            let mut pairs = Vec::new();
            for pair in headers[1..].chunks_exact(2) {
                pairs.push((string(&pair[0])?.to_owned(), string(&pair[1])?.to_owned()));
            }
            let boolean = |index| -> Result<bool> { Ok(bool_text(&fields[index])? == "true") };
            Ok(Self {
                header: Header {
                    uuid: head.uuid,
                    name: head.name,
                    synonym,
                    comment: head.comment,
                },
                predefined: boolean(Layout::PREDEFINED)?,
                auto_connect: boolean(Layout::AUTO_CONNECT)?,
                server_url: string(&fields[Layout::SERVER_URL])?.to_owned(),
                user: string(&fields[Layout::USER])?.to_owned(),
                password: string(&fields[Layout::PASSWORD])?.to_owned(),
                headers: pairs,
                use_os_proxy: boolean(Layout::OS_PROXY)?,
                use_os_authentication: boolean(Layout::OS_AUTHENTICATION)?,
                timeout: number(&fields[Layout::TIMEOUT])?,
            })
        })()
        .context("WebSocketClient descriptor")
    }

    pub fn to_xml(&self) -> Result<Element> {
        let head = header(&self.header.to_brace())?;
        let mut properties = el("Properties").children(header_elements(&head)?);
        properties = properties
            .child(leaf(
                "Predefined",
                if self.predefined { "true" } else { "false" },
            ))
            .child(leaf(
                "AutoConnect",
                if self.auto_connect { "true" } else { "false" },
            ))
            .child(leaf("ServerURL", xml_text(&self.server_url)))
            .child(leaf("User", xml_text(&self.user)))
            .child(leaf("Password", xml_text(&self.password)));
        let mut headers = el("Headers").attr("xsi:type", "xr:ValueList");
        for (key, value) in &self.headers {
            headers = headers.child(
                el("xr:Item")
                    .child(el("xr:Presentation"))
                    .child(leaf("xr:CheckState", "0"))
                    .child(
                        el("xr:Value")
                            .attr("xsi:type", "v8:KeyAndValue")
                            .child(leaf("v8:Key", xml_text(key)).attr("xsi:type", "xs:string"))
                            .child(leaf("v8:Value", xml_text(value)).attr("xsi:type", "xs:string")),
                    ),
            );
        }
        properties = properties
            .child(headers)
            .child(leaf(
                "UseOSProxy",
                if self.use_os_proxy { "true" } else { "false" },
            ))
            .child(leaf(
                "UseOSAuthentication",
                if self.use_os_authentication {
                    "true"
                } else {
                    "false"
                },
            ))
            .child(leaf("Timeout", self.timeout.to_string()));
        Ok(el(Layout::KIND)
            .attr("uuid", self.header.uuid.clone())
            .child(properties))
    }

    pub fn names(&self) -> ObjectNames {
        ObjectNames {
            uuid: self.header.uuid.clone(),
            full_name: format!("{}.{}", Layout::KIND, self.header.name),
            children: Vec::new(),
            types: Vec::new(),
        }
    }
}
