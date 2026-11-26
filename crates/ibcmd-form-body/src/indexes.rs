//! The per-export indexes a form is parsed against beside the plain
//! reference maps: the targets of standard commands, the fields of
//! information registers (from `src/mssql_dump/mod.rs`) and the DCS type
//! index (from `src/mssql_dump/dcs.rs`). Their builders stay in the root.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct MetadataCommandReference {
    pub kind: String,
    pub name: String,
    /// How many members the target's own `<BasedOn>` list declares, or `None`
    /// when this reader cannot name the slot for the target's kind. A target
    /// that declares none has no `CreateBasedOn` standard command at all, and
    /// the platform keeps the raw `code:uuid` sentinel for it exactly as it
    /// does for `use_standard_commands == false`. Evidence: over the eight
    /// stand corpora every one of the 5 123 `<Command>`/`<CommandName>` values
    /// naming a `.StandardCommand.CreateBasedOn` names a target whose
    /// `<BasedOn>` is non-empty, and the five ERP УХ 3.2.12.6 documents whose
    /// `<BasedOn/>` is empty -- `УдалитьЗаявкаНаКорректировкуЛимитов`,
    /// `УдалитьКорректировкаЛимитов`, `СтрокаПланаЗакупок`,
    /// `ОбоснованиеТребованийКЗакупочнойПроцедуре` and `ЗаявкаНаРасход` -- are
    /// carried by 22 buttons the platform writes as `2:<uuid>`.
    pub based_on_declared: Option<usize>,
    /// How many owners the target's own `<Owners>` list declares, or `None`
    /// when this reader cannot name the slot for the target's kind. A catalog
    /// that declares none has nothing to be opened by, so it has no
    /// `OpenByValue` standard command and the platform keeps the raw
    /// `code:uuid` sentinel -- the same shape as `based_on_declared`.
    ///
    /// Evidence: over the eight stand corpora every one of the 112
    /// `<Command>`/`<CommandName>` values naming a
    /// `Catalog.X.StandardCommand.OpenByValue` names a catalog whose
    /// `<Owners>` declares at least one owner, and ERP УХ 3.2.12.6 keeps
    /// `Catalog.СхемаДоступностиРеквизитов` -- whose `<Owners/>` is empty --
    /// as `4:bdd23491-0b0b-49b7-bda4-8a0b0870f4fd` in
    /// `Catalogs/ЭтапыУниверсальныхПроцессов/Forms/ФормаЭлемента`.
    pub owners_declared: Option<usize>,
    /// Whether the target information register declares
    /// `<WriteMode>RecorderSubordinate</WriteMode>`, or `None` for every other
    /// kind. A register written independently has no recorder, so it has no
    /// `OpenByRecorder` standard command.
    ///
    /// Evidence: over the eight stand corpora all ten
    /// `<Command>`/`<CommandName>` values naming an
    /// `InformationRegister.X.StandardCommand.OpenByRecorder` name a register
    /// that declares `RecorderSubordinate`, and none names an `Independent`
    /// one; ERP УХ 3.2.12.6 keeps
    /// `InformationRegister.НастройкаРаспределенияПоНаправлениямДеятельности`
    /// (`Independent`) as `2:7f22e01e-6dbf-4a58-a96b-b455c935ab6b` in
    /// `Catalogs/Организации/Forms/ФормаЭлемента`.
    pub recorder_subordinate: Option<bool>,
    /// Whether the target metadata object itself declares
    /// `<UseStandardCommands>true</UseStandardCommands>`. Both command-interface
    /// readers (this object's own `Ext/CommandInterface.xml` and a form's
    /// embedded `<CommandInterface>`) must decline to synthesize a
    /// `.StandardCommand.X` name when this is `false`: the platform keeps the
    /// raw `code:uuid` sentinel instead, because the standard command does not
    /// exist on that target. Evidence: ERP UH `MDM_Management`,
    /// `Catalog.ТабличныеЧастиБД` (`UseStandardCommands=false`) is kept as
    /// `0:04c1ab92-f4fc-40e9-b0d6-a1c57a5a8b47` in
    /// `Subsystems/.../Ext/CommandInterface.xml`, and `Catalog.СправочникиБД`
    /// (also `false`) is kept as `4:67b8886f-f30d-480c-abb8-e3712f3c169c` in
    /// `Catalogs/ТипыБазДанных/Forms/ФормаЭлемента/Ext/Form.xml`. Defaults to
    /// `true` (today's behaviour) for every kind the index cannot yet prove a
    /// value for -- only `Catalog` is evidenced so far.
    pub use_standard_commands: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InformationRegisterFieldReference {
    pub field_reference: String,
    pub value_owner_references: BTreeSet<String>,
}

pub type InformationRegisterFieldReferenceIndex =
    BTreeMap<String, Vec<InformationRegisterFieldReference>>;

/// Register uuid -> its master dimension names in declaration order.
pub type InformationRegisterMasterDimensionIndex = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DcsTypeResolution {
    KeepId,
    Type { qname: String },
    TypeSet { qname: String },
}

pub type DcsTypeIndex = BTreeMap<String, DcsTypeResolution>;
