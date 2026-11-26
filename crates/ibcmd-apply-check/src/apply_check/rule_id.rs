//! Stable identifiers of the rules and check steps that make a [`Reason`]
//! (see `model`).
//!
//! A reason used to carry its rule only as text inside `change`
//! (`9 -> 12 (a property no rule covers)`), so whoever had to act on a class
//! of reasons matched words. An id is the machine-readable form: it never
//! changes for a rule that keeps its meaning, and the text (`text`) is what
//! the reason's `change` still prints. `id()` is the string that goes into
//! the JSON report and into the tests' expectations.
//!
//! Two kinds of rules share the enumeration:
//! * the descriptor rules of `rules.rs`, which judge one change of an object
//!   (their `text` is the wording the reasons have always had);
//! * the steps of the check that judge something that is not a property
//!   (a row that cannot be read, a stage that lists a file it does not
//!   hold): their reasons carry a text of their own, `text` says what the
//!   step is.

use serde::{Serialize, Serializer};

macro_rules! rule_ids {
    ($($(#[$doc:meta])* $variant:ident => $id:literal, $text:literal;)*) => {
        /// The rule or the step of the check that made a reason.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub enum RuleId {
            /// A reason built without a rule (a literal that predates ids):
            /// every consumer must treat it as unknown.
            #[default]
            Unspecified,
            $($(#[$doc])* $variant,)*
        }

        impl RuleId {
            /// Every id, for the tests that walk them.
            pub const ALL: &'static [RuleId] = &[RuleId::Unspecified, $(RuleId::$variant),*];

            /// The stable name (`column-added-or-dropped`).
            pub fn id(self) -> &'static str {
                match self {
                    RuleId::Unspecified => "unspecified",
                    $(RuleId::$variant => $id,)*
                }
            }

            /// What the rule says, in the words the reasons print.
            pub fn text(self) -> &'static str {
                match self {
                    RuleId::Unspecified => "a reason without a rule",
                    $(RuleId::$variant => $text,)*
                }
            }
        }
    };
}

rule_ids! {
    // The descriptor rules (rules.rs).
    KindOwnsNoStoredData => "kind-owns-no-stored-data", "the kind owns no stored data";
    ObjectChanged => "object-changed", "the object itself changed";
    ConfigurationObjectsChanged => "configuration-objects-changed", "the objects of the configuration changed";
    GeneratedTypesChanged => "generated-types-changed", "generated types of the object changed";
    DescriptorPartNotCovered => "descriptor-part-not-covered", "a part of the descriptor no rule covers";
    PropertiesChanged => "properties-changed", "the properties changed";
    StandardAttributePresentation => "standard-attribute-presentation", "presentation of a standard attribute";
    StandardAttributePropertyNotCovered => "standard-attribute-property-not-covered", "a standard attribute's property no rule covers";
    PresentationProperty => "presentation-property", "presentation or behaviour property";
    ScheduledJobStored => "scheduled-job-stored", "the platform keeps the scheduled jobs in a table of its own";
    PropertyNotCovered => "property-not-covered", "a property no rule covers";
    ChildObjectsChanged => "child-objects-changed", "the child objects changed";
    ChildKindNotCovered => "child-kind-not-covered", "a kind of child object no rule covers";
    InterfaceRows => "interface-rows", "commands, forms and templates are rows and lists of names";
    ColumnAddedOrDropped => "column-added-or-dropped", "a column is added or dropped";
    ColumnsReordered => "columns-reordered", "the columns changed order";
    AttributePresentation => "attribute-presentation", "presentation or behaviour of an attribute";
    AttributePropertyNotCovered => "attribute-property-not-covered", "a property of an attribute no rule covers";
    AttributePartNotCovered => "attribute-part-not-covered", "a part of an attribute no rule covers";
    TabularSectionAddedDroppedMoved => "tabular-section-added-dropped-moved", "a tabular section is added, dropped or moved";
    TabularSectionPresentation => "tabular-section-presentation", "presentation of a tabular section";
    TabularSectionStandardAttributePresentation => "tabular-section-standard-attribute-presentation", "presentation of a tabular section's standard attribute";
    TabularSectionStandardAttributeNotCovered => "tabular-section-standard-attribute-not-covered", "a tabular section's standard attribute no rule covers";
    TabularSectionPropertyNotCovered => "tabular-section-property-not-covered", "a property of a tabular section no rule covers";
    TabularSectionColumnAddedDroppedMoved => "tabular-section-column-added-dropped-moved", "a column of a tabular section is added, dropped or moved";
    TabularSectionAttributePresentation => "tabular-section-attribute-presentation", "presentation or behaviour of a tabular section's attribute";
    TabularSectionAttributePropertyNotCovered => "tabular-section-attribute-property-not-covered", "a property of a tabular section's attribute no rule covers";
    TabularSectionPartNotCovered => "tabular-section-part-not-covered", "a part of a tabular section no rule covers";
    EnumValueAddedOrDropped => "enum-value-added-or-dropped", "a value of an enumeration is added or dropped";
    EnumValuesReordered => "enum-values-reordered", "the values of an enumeration changed order";
    EnumValuePresentation => "enum-value-presentation", "presentation of an enumeration value";
    EnumValuePartNotCovered => "enum-value-part-not-covered", "a part of an enumeration value no rule covers";
    ObjectWithStorageAddedOrDropped => "object-with-storage-added-or-dropped", "an object that owns tables or stored data is added or dropped";

    // The steps of the check (check.rs, dbtree.rs, trees.rs, descriptor.rs).
    KindChanged => "kind-changed", "the same object is another kind on the two sides";
    RowDecodesToSameXml => "row-decodes-to-same-xml", "the row differs in bytes but both sides decode to the same XML, and no proof of a format upgrade was available";
    RowFormatUpgradeUnproven => "row-format-upgrade-unproven", "the row differs in bytes, both sides decode to the same XML, and the difference is not a known format upgrade";
    RowUndecodable => "row-undecodable", "a changed row cannot be decoded";
    DescriptorKindUnknown => "descriptor-kind-unknown", "the check cannot tell what kind of object the descriptor is";
    DescriptorsUnreadable => "descriptors-unreadable", "the descriptors could not all be read";
    NoVersionsRow => "no-versions-row", "the ConfigSave holds no versions row";
    VersionsListsUnknownFile => "versions-lists-unknown-file", "the staged versions row lists a file that neither Config nor ConfigSave holds";
    RootRowChanged => "root-row-changed", "the root row differs";
    VersionRowChanged => "version-row-changed", "the version row differs";
    OnlineUpdateRow => "online-update-row", "the ConfigSave holds a row of the platform's online-update state";
    DescriptorNotListed => "descriptor-not-listed", "the ConfigSave holds a descriptor that the staged versions row does not list";
    StagedDescriptorUnreadable => "staged-descriptor-unreadable", "the staged descriptor could not be read";
    NewDescriptorUnplaced => "new-descriptor-unplaced", "a new descriptor of a kind the check cannot place";
    RemovedDescriptorUnplaced => "removed-descriptor-unplaced", "a removed descriptor of a kind the check cannot place";
    BodyRowNotListed => "body-row-not-listed", "the ConfigSave holds a row that the staged versions row does not list";
    RowUnknown => "row-unknown", "the ConfigSave holds a row the check does not know";
    RemovedRowUnknown => "removed-row-unknown", "a removed row the check does not know";
    DeletedRowNotEmpty => "deleted-row-not-empty", "the deleted row lists files";
    BodyRowUnreadable => "body-row-unreadable", "a body row could not be read";
    BodyContentChanged => "body-content-changed", "the content of a body row that carries data changed";
    BodyRowAdded => "body-row-added", "a body row that carries data was added";
    BodyRowRemoved => "body-row-removed", "a body row was removed";
    StagedVersionsPartial => "staged-versions-partial", "the staged versions row lists a file the ConfigSave does not carry";
    SameObjectTwice => "same-object-twice", "two files of a tree hold the same object";
    ComparisonFailed => "comparison-failed", "the comparison could not run";
    TreeFileUnreadable => "tree-file-unreadable", "a file of a tree could not be read";
    TreeFileUnknown => "tree-file-unknown", "a file of a tree the check does not know";
}

impl Serialize for RuleId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.text())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_id_is_unique_kebab_case_and_has_a_text() {
        let mut seen = HashSet::new();
        for rule in RuleId::ALL {
            let id = rule.id();
            assert!(seen.insert(id), "{id} is used twice");
            assert!(
                !id.is_empty()
                    && id.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                    && !id.starts_with('-')
                    && !id.ends_with('-'),
                "{id}"
            );
            assert!(!rule.text().is_empty(), "{id}");
        }
    }

    #[test]
    fn the_id_serializes_as_its_name() {
        assert_eq!(
            serde_json::to_string(&RuleId::ColumnAddedOrDropped).unwrap(),
            "\"column-added-or-dropped\""
        );
        assert_eq!(RuleId::default(), RuleId::Unspecified);
    }
}
