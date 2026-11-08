/// The hierarchy-panel member appended by the native revision-73 table.
/// Native 8.5.1.1150 corpus records and isolated 8.5.1.1529 controls use a
/// 28-member tail: code 0 omits the XML element, code 1 writes `None`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FormTableHierarchyPanelLocationSchema;

impl FormTableHierarchyPanelLocationSchema {
    pub const TABLE_XML_NAME: &'static str = "Table";
    pub const REVISION: &'static str = "73";
    pub const XML_NAME: &'static str = "HierarchyPanelLocation";
    pub const XML_NAMESPACE: &'static str = "http://v8.1c.ru/8.3/xcf/logform";
    pub const VALUES: &'static [(&'static str, Option<&'static str>)] =
        &[("0", None), ("1", Some("None"))];

    pub const fn tail_slot() -> usize {
        2
    }

    pub fn from_raw_layout(revision: &str, tail_members: usize) -> Option<Self> {
        (revision == Self::REVISION && tail_members == 28).then_some(Self)
    }

    /// The native 2.21 order places this member after SearchControlLocation.
    /// Keep the inherited order of every other child unchanged.
    pub fn complete_xml_child_order<'a>(
        parent: &str,
        existing: &'a [&'static str],
    ) -> std::borrow::Cow<'a, [&'static str]> {
        if parent != Self::TABLE_XML_NAME || existing.contains(&Self::XML_NAME) {
            return std::borrow::Cow::Borrowed(existing);
        }
        let Some(anchor) = existing
            .iter()
            .position(|name| *name == "SearchControlLocation")
        else {
            return std::borrow::Cow::Borrowed(existing);
        };
        let mut order = existing.to_vec();
        order.insert(anchor + 1, Self::XML_NAME);
        std::borrow::Cow::Owned(order)
    }
}
