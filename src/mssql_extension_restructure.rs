//! SQL adapter for measured additive extension schema updates (#582).
//!
//! The exported metadata is the semantic boundary: catalog/document records
//! use the existing typed S1 adapters, while register properties are read by
//! their XML names. No new native metadata slot numbers are introduced here.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ibcmd_core::storage::StorageImage;
use ibcmd_restructure::extension::{
    NameRequest, NameStep, SchemaPlan, TableChange, allocate_extension_names,
};
use ibcmd_restructure::scheduled_jobs::{Job as ScheduledJob, Service as JobService};
use ibcmd_schema::extension_schema::{
    ExtensionScheduledJobsLayout as JobsLayout, ExtensionServiceScopeLayout as ScopeLayout,
};
use sha2::{Digest, Sha256};

use crate::metadata_model::DescriptorContext;
use crate::metadata_model::brace::{Brace, parse_row};
use crate::metadata_model::xml::{Element, MetadataXml};
use crate::mssql_config_apply::model::{hex_upper, quote_ident, quote_string};
use crate::mssql_extension_stage::{ExtensionStagePlan, extension_namespace_prefix};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;
use crate::restructure::caches::facts::ObjectFacts;
use crate::restructure::caches::members::{Member, Members};
use crate::restructure::caches::names_tables::NamesTables;
use crate::restructure::common::CommonAttributes;
use crate::restructure::entry::{EntryInput, Kind, RefTables, attribute_types, main_entry};
use crate::restructure::names::{DbNames, inflate, version_row};
use crate::restructure::schema::{DbSchema, FieldEntry, IndexEntry, TableView};
use crate::restructure::storage::EMPTY_GENERATION;
use crate::sql::SqlExec;

const TABLELESS: &[&str] = &[
    "DataProcessors",
    "CommonForms",
    "CommonModules",
    "CommonPictures",
    "CommonTemplates",
    "Roles",
    "HTTPServices",
    "Subsystems",
];
const DATA: &[&str] = &[
    "Catalogs",
    "Documents",
    "Enums",
    "InformationRegisters",
    "ScheduledJobs",
];

#[derive(Clone, Debug)]
pub struct ExtensionRestructurePlan {
    sql: String,
    objects: Vec<String>,
    tables: Vec<String>,
    pub after_schema: Vec<u8>,
    pub after_names: Vec<u8>,
    pub after_system_names: Vec<u8>,
    pub after_scheduled_jobs: Option<Vec<u8>>,
}
impl ExtensionRestructurePlan {
    pub fn phase_sql(&self) -> &str {
        &self.sql
    }
    pub fn touched_tables(&self) -> Vec<String> {
        self.tables.clone()
    }
    pub fn objects(&self) -> Vec<String> {
        self.objects.clone()
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!("ibcmd-ext-schema-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root)?;
        Ok(Self(root))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct Object {
    family: String,
    kind: String,
    uuid: String,
    name: String,
    element: Element,
    row: Brace,
}
impl Object {
    fn properties(&self) -> Result<&Element> {
        self.element
            .child("Properties")
            .context("metadata has no Properties")
    }
    fn children(&self) -> &[Element] {
        self.element
            .child("ChildObjects")
            .map(|c| c.children.as_slice())
            .unwrap_or_default()
    }
    fn fields(&self) -> Vec<&Element> {
        let roles = if self.kind == "InformationRegister" {
            &["Dimension", "Resource", "Attribute"][..]
        } else {
            &["Attribute"][..]
        };
        roles
            .iter()
            .flat_map(|role| self.children().iter().filter(move |c| c.name == *role))
            .collect()
    }
    fn field_ids(&self) -> Result<Vec<String>> {
        self.fields().into_iter().map(|e| id(e)).collect()
    }
    fn adopted(&self) -> Result<bool> {
        Ok(self.properties()?.child_text("ObjectBelonging") == Some("Adopted"))
    }
    fn table_kind(&self) -> &str {
        match self.kind.as_str() {
            "Catalog" => "Reference",
            "Document" => "Document",
            "Enum" => "Enum",
            "ScheduledJob" => JobsLayout::TABLE_KIND,
            _ => "InfoRg",
        }
    }
    fn label(&self) -> String {
        format!("{}.{}", self.kind, self.name)
    }
}

fn id(e: &Element) -> Result<String> {
    let s = e.attr("uuid").context("metadata child has no uuid")?;
    uuid::Uuid::parse_str(s).context("metadata uuid is invalid")?;
    Ok(s.to_ascii_lowercase())
}
fn equal(a: &Element, b: &Element) -> bool {
    a.name == b.name
        && a.prefix == b.prefix
        && a.attrs == b.attrs
        && a.namespaces == b.namespaces
        && a.text.trim() == b.text.trim()
        && a.children.len() == b.children.len()
        && a.children.iter().zip(&b.children).all(|(a, b)| equal(a, b))
}
fn without(e: &Element, names: &[&str]) -> Element {
    let mut e = e.clone();
    e.children.retain(|c| !names.contains(&c.name.as_str()));
    e
}
fn bytes(image: &StorageImage) -> BTreeMap<String, Vec<u8>> {
    image
        .entries()
        .iter()
        .map(|e| {
            (
                e.logical_key().as_str().to_ascii_lowercase(),
                e.packed_payload().to_vec(),
            )
        })
        .collect()
}

fn objects(root: &Path, image: &StorageImage) -> Result<Vec<Object>> {
    let rows = bytes(image);
    let cfg = MetadataXml::parse(&std::fs::read(root.join("Configuration.xml"))?)?;
    let children = cfg
        .object()?
        .child("ChildObjects")
        .context("configuration has no ChildObjects")?;
    let mut out = Vec::new();
    for (family, kind) in [
        ("Catalogs", "Catalog"),
        ("Documents", "Document"),
        ("Enums", "Enum"),
        ("InformationRegisters", "InformationRegister"),
        ("ScheduledJobs", "ScheduledJob"),
    ] {
        for c in children.children_named(kind) {
            let doc = MetadataXml::parse(&std::fs::read(
                root.join(family).join(format!("{}.xml", c.text.trim())),
            )?)?;
            let e = doc.object()?.clone();
            let uuid = id(&e)?;
            let row = rows
                .get(&uuid)
                .with_context(|| format!("{} {} has no descriptor row", kind, c.text))?;
            let plain = inflate(row).unwrap_or_else(|_| row.clone());
            out.push(Object {
                family: family.into(),
                kind: kind.into(),
                uuid,
                name: c.text.trim().into(),
                element: e,
                row: parse_row(&plain)?,
            });
        }
    }
    Ok(out)
}

fn validate_identities(old: &[Object], new: &[Object]) -> Result<()> {
    for before in old {
        if !new.iter().any(|after| {
            after.uuid == before.uuid
                && after.kind == before.kind
                && after.family == before.family
                && after.name == before.name
        }) {
            bail!(
                "extension update removes, renames or replaces the identity of {}",
                before.label()
            );
        }
    }
    Ok(())
}

// The schema planner proves data descriptors. Every skipped source asset still
// has to satisfy the source compiler's existing transition policy.
fn require_source_asset_transition(path: &str, added: bool, removed: bool) -> Result<()> {
    let diff = crate::load::TreeDiff {
        changed: if !added && !removed {
            vec![path.to_owned()]
        } else {
            vec![]
        },
        added: if added { vec![path.to_owned()] } else { vec![] },
        removed: if removed {
            vec![path.to_owned()]
        } else {
            vec![]
        },
    };
    let refused = crate::mssql_extension_tree_load::structural_refusals(&diff, true);
    if !refused.is_empty() {
        bail!(
            "extension schema update cannot override source-transition refusal: {}",
            refused.join(", ")
        );
    }
    Ok(())
}

/// Inspect every changed path: a schema planner never turns ignored families
/// into an accidental permission to publish an unmodeled metadata change.
fn classify(base: &Path, next: &Path, old: &[Object], new: &[Object]) -> Result<Vec<String>> {
    validate_identities(old, new)?;
    let a = crate::load::relative_files(base)?;
    let b = crate::load::relative_files(next)?;
    let mut changed = Vec::new();
    for path in a.union(&b) {
        if path == "ConfigDumpInfo.xml" {
            continue;
        }
        let left = std::fs::read(base.join(path)).ok();
        let right = std::fs::read(next.join(path)).ok();
        if left == right {
            continue;
        }
        if path == "Configuration.xml" {
            let x = MetadataXml::parse(left.as_deref().context("configuration root removed")?)?;
            let y = MetadataXml::parse(right.as_deref().context("configuration root missing")?)?;
            let x = x.object()?;
            let y = y.object()?;
            ibcmd_xml::extension_root::extension_root_edit(
                left.as_deref().context("configuration root removed")?,
                right.as_deref().context("configuration root missing")?,
                |tag| crate::mssql_dump::root_family_folder(tag).is_some(),
            )
            .map_err(|changed| {
                anyhow::anyhow!(
                    "unsupported extension root transition: {}",
                    changed.join(", ")
                )
            })?;
            let xp = x.child("ChildObjects").context("root child list missing")?;
            let yp = y.child("ChildObjects").context("root child list missing")?;
            for c in &xp.children {
                if !yp.children.iter().any(|n| equal(c, n)) {
                    bail!(
                        "extension update removes a root child {} {}",
                        c.name,
                        c.text
                    );
                }
            }
            continue;
        }
        let parts: Vec<_> = path.split('/').collect();
        let family = parts.first().copied().unwrap_or_default();
        if path.contains("/Ext/Predefined") {
            bail!("predefined-data edits require a separate data migration: {path}");
        }
        if DATA.contains(&family) {
            // A separate schema delta does not prove the lifecycle of a
            // retained object's assets. Reuse the source gate, including its
            // explicit form-module exception, rather than another path policy.
            if (left.is_none() || right.is_none())
                && (path.contains("/Ext/")
                    || parts
                        .iter()
                        .any(|part| matches!(*part, "Forms" | "Templates")))
            {
                let is_new_owner = new.iter().any(|object| {
                    object.family == family
                        && Some(object.name.as_str()) == parts.get(1).copied()
                        && !old.iter().any(|before| before.uuid == object.uuid)
                });
                if !is_new_owner {
                    require_source_asset_transition(path, left.is_none(), right.is_none())?;
                }
            }
            if parts.len() == 2 && path.ends_with(".xml") {
                let n = new
                    .iter()
                    .find(|o| format!("{}/{}.xml", o.family, o.name) == *path)
                    .with_context(|| format!("extension update removes data object {path}"))?;
                if let Some(o) = old.iter().find(|o| o.uuid == n.uuid) {
                    validate_object_update(o, n)?;
                } else {
                    if n.adopted()?
                        || !matches!(
                            n.kind.as_str(),
                            "Catalog" | "InformationRegister" | "ScheduledJob"
                        )
                    {
                        bail!("new {} is not a measured own data object", n.label());
                    }
                    validate_new(n)?;
                }
                changed.push(n.uuid.clone());
                continue;
            }
            if path.contains("/Ext/") {
                if family == "ScheduledJobs" {
                    let owner = new
                        .iter()
                        .find(|o| {
                            o.kind == "ScheduledJob"
                                && Some(o.name.as_str()) == parts.get(1).copied()
                        })
                        .context("scheduled-job resource has no owner")?;
                    validate_new(owner)?;
                }
                continue;
            }
            if parts.iter().any(|p| matches!(*p, "Forms" | "Templates")) {
                continue;
            }
            bail!("extension schema update has an unsupported data-object path: {path}");
        }
        if TABLELESS.contains(&family) || (family == "Ext" && path.ends_with(".bsl")) {
            require_source_asset_transition(path, left.is_none(), right.is_none())?;
            continue;
        }
        bail!("extension schema update has an unmodeled changed path: {path}");
    }
    Ok(changed)
}

fn validate_new(n: &Object) -> Result<()> {
    let p = n.properties()?;
    let fields = n.field_ids()?;
    if fields.iter().collect::<BTreeSet<_>>().len() != fields.len() {
        bail!("{} repeats a field identity", n.label());
    }
    if n.kind == "ScheduledJob" && p.child_text("Predefined") != Some("false") {
        bail!(
            "{}: predefined scheduled-job instances require an unmodeled native ActivationCondition writer; nothing was staged or published",
            n.label()
        );
    }
    if p.child_text("DataHistory") == Some("Use") {
        bail!("{} enables unmeasured data history", n.label());
    }
    if n.kind == "InformationRegister" {
        for (prop, value) in [
            ("InformationRegisterPeriodicity", "Nonperiodical"),
            ("WriteMode", "Independent"),
            ("EnableTotalsSliceFirst", "false"),
            ("EnableTotalsSliceLast", "false"),
        ] {
            if p.child_text(prop) != Some(value) {
                bail!("{} has unmeasured {prop}", n.label());
            }
        }
        if !n.fields().iter().any(|c| c.name == "Dimension") {
            bail!("{} has no dimension key", n.label());
        }
        for f in n.fields() {
            let p = f
                .child("Properties")
                .context("register field has no properties")?;
            if p.child_text("Indexing").is_some_and(|s| s != "DontIndex") {
                bail!("{} has unmeasured field indexing", n.label());
            }
        }
    }
    for c in n.children() {
        if !matches!(
            c.name.as_str(),
            "Attribute" | "Dimension" | "Resource" | "Form" | "Template" | "Command" | "EnumValue"
        ) {
            bail!("{} has an unmeasured child {}", n.label(), c.name);
        }
    }
    Ok(())
}

fn validate_object_update(o: &Object, n: &Object) -> Result<()> {
    if equal(&o.element, &n.element) {
        return Ok(());
    }
    if o.adopted()? || n.adopted()? {
        bail!("structural edits of adopted {} are not measured", n.label());
    }
    if !equal(
        &without(&o.element, &["ChildObjects"]),
        &without(&n.element, &["ChildObjects"]),
    ) {
        bail!(
            "{} changes existing object properties or generated types",
            n.label()
        );
    }
    for c in o.children() {
        if !n.children().iter().any(|next| equal(c, next)) {
            bail!("{} changes or removes an existing {}", n.label(), c.name);
        }
    }
    for c in n.children() {
        if o.children().iter().any(|old| equal(c, old)) {
            continue;
        }
        match (n.kind.as_str(), c.name.as_str()) {
            ("Catalog" | "Document", "Attribute")
            | ("Enum", "EnumValue")
            | (_, "Form" | "Template" | "Command") => {}
            _ => bail!("{} adds an unmeasured child {}", n.label(), c.name),
        }
    }
    if n.kind == "Enum" {
        let a: Vec<_> = o
            .children()
            .iter()
            .filter(|e| e.name == "EnumValue")
            .collect();
        let b: Vec<_> = n
            .children()
            .iter()
            .filter(|e| e.name == "EnumValue")
            .collect();
        if b.len() < a.len() || a.iter().zip(&b).any(|(a, b)| !equal(a, b)) {
            bail!("enum values must retain their existing order and identity");
        }
    }
    validate_new(n)
}

#[derive(Clone)]
pub struct Param {
    pub name: String,
    pub size: i64,
    pub part: i32,
    pub data: Vec<u8>,
}
#[derive(Clone)]
pub struct Schema {
    pub id: i64,
    pub status: i64,
    pub current: Vec<u8>,
    pub created: Vec<u8>,
    pub dropped: Vec<u8>,
}
pub struct ExtensionRestructureState {
    pub params: Vec<Param>,
    pub schemas: Vec<Schema>,
    pub services: Vec<(Vec<u8>, Vec<u8>, i64, i64)>,
}
/// The native global service context is copied from the selected registry
/// row. Nullable separator values are not replaced with guessed defaults.
struct ServiceScope {
    columns: Vec<String>,
    use_columns: Vec<String>,
    extension_id: [u8; 16],
}
impl ServiceScope {
    fn from_schemas(schemas: &[Schema], extension_id: [u8; 16]) -> Result<Self> {
        let main = schemas
            .iter()
            .find(|s| s.id == 0)
            .context("main SchemaStorage row 0 is required for extension service context")?;
        let schema = DbSchema::parse(&main.current)?;
        let service = schema.named(ScopeLayout::SERVICE_TABLE)?.fields()?;
        let registry = schema.named(ScopeLayout::REGISTRY_TABLE)?.fields()?;
        for required in [
            "ExtDataID",
            "RestructData",
            "RestructDataInt",
            "RestructDataType",
        ] {
            if service.iter().filter(|f| f.name == required).count() != 1 {
                bail!("extension service schema lacks {required}");
            }
        }
        for required in ["ID", "ExtensionScope"] {
            if registry.iter().filter(|f| f.name == required).count() != 1 {
                bail!("extension registry schema lacks {required}");
            }
        }
        let mut columns = Vec::new();
        let mut use_columns = Vec::new();
        for field in &service {
            if [
                "ExtDataID",
                "RestructData",
                "RestructDataInt",
                "RestructDataType",
            ]
            .contains(&field.name.as_str())
            {
                continue;
            }
            if !ScopeLayout::is_context_field(&field.name) {
                bail!("unmodeled extension service field {}", field.name);
            }
            let corresponding = registry
                .iter()
                .find(|r| r.name == field.name)
                .context("extension registry lacks a service context field")?;
            if corresponding != field {
                bail!("extension registry and service context field types disagree");
            }
            let physical = crate::restructure::schema::field_columns(field)?;
            if field.name.starts_with(ScopeLayout::USE_PREFIX) {
                if physical.len() != 1
                    || physical[0].sql_type != crate::restructure::schema::SqlType::Binary(1)
                    || physical[0].nullable
                {
                    bail!("unmodeled extension data-separation use flag");
                }
                use_columns.push(quote_ident(&physical[0].name)?);
            }
            for column in physical {
                columns.push(quote_ident(&column.name)?);
            }
        }
        Ok(Self {
            columns,
            use_columns,
            extension_id,
        })
    }
    fn registry_predicate(&self) -> String {
        let mut value = format!(
            "ctx._IDRRef=0x{} AND ctx._ExtensionScope=1",
            hex_upper(&self.extension_id)
        );
        for col in &self.use_columns {
            write!(value, " AND ctx.{col}=0x00").unwrap();
        }
        value
    }
    fn predicate(&self, alias: &str) -> String {
        let mut value = format!(
            "EXISTS(SELECT 1 FROM dbo._ExtensionsInfo ctx WITH(UPDLOCK,HOLDLOCK) WHERE {}",
            self.registry_predicate()
        );
        for col in &self.columns {
            write!(
                value,
                " AND (ctx.{col}={alias}.{col} OR (ctx.{col} IS NULL AND {alias}.{col} IS NULL))"
            )
            .unwrap();
        }
        value.push(')');
        value
    }
    fn insert_columns(&self) -> String {
        self.columns.iter().map(|c| format!(",{c}")).collect()
    }
    fn insert_values(&self) -> String {
        self.columns.iter().map(|c| format!(",ctx.{c}")).collect()
    }
}

fn state(
    sql: &SqlExec,
    database: &str,
    extension_id: [u8; 16],
) -> Result<ExtensionRestructureState> {
    let client = sql
        .client()
        .context("extension restructuring requires the built-in SQL client")?;
    let db = quote_ident(database)?;
    let mut params = Vec::new();
    client.read_rows(&format!("USE {db}; SELECT FileName,DataSize,PartNo,BinaryData FROM dbo.Params WHERE FileName LIKE N'DBNames%' OR FileName=N'a07b62f0-1f01-484a-93d9-d42764cedac0.si' ORDER BY FileName,PartNo"),&[],&mut|mut r|{params.push(Param{name:r.take_text(0)?,size:r.i64(1)?,part:r.i64(2)?.try_into()?,data:r.take_binary(3)?});Ok(())})?;
    let mut schemas = Vec::new();
    client.read_rows(&format!("USE {db}; SELECT SchemaID,Status,CurrentSchema,NewGenCreated,NewGenDropped FROM dbo.SchemaStorage ORDER BY SchemaID"),&[],&mut|mut r|{schemas.push(Schema{id:r.i64(0)?,status:r.i64(1)?,current:r.take_binary(2)?,created:r.take_binary(3)?,dropped:r.take_binary(4)?});Ok(())})?;
    let scope = ServiceScope::from_schemas(&schemas, extension_id)?;
    let mut registry_count = 0;
    client.read_rows(
        &format!(
            "USE {db}; SELECT COUNT_BIG(*) FROM dbo._ExtensionsInfo ctx WHERE {}",
            scope.registry_predicate()
        ),
        &[],
        &mut |r| {
            registry_count = r.i64(0)?;
            Ok(())
        },
    )?;
    if registry_count != 1 {
        bail!("extension restructuring requires exactly one global Infobase registry context");
    }
    let mut services = Vec::new();
    client.read_rows(&format!("USE {db}; SELECT _ExtDataID,_RestructData,_RestructDataInt,_RestructDataType FROM dbo._ExtensionsRestruct r WHERE {} ORDER BY _ExtDataID",scope.predicate("r")),&[],&mut|mut r|{services.push((r.take_binary(0)?,r.take_binary(1)?,r.i64(2)?,r.i64(3)?));Ok(())})?;
    if params.len() > 4096 || schemas.len() > 16 || services.len() > 4096 {
        bail!("extension schema state exceeds bounded row counts");
    }
    Ok(ExtensionRestructureState {
        params,
        schemas,
        services,
    })
}
fn param<'a>(s: &'a ExtensionRestructureState, name: &str) -> Result<&'a Param> {
    let v: Vec<_> = s.params.iter().filter(|p| p.name == name).collect();
    if v.len() != 1 || v[0].part != 0 || v[0].size != v[0].data.len() as i64 {
        bail!("missing, multipart or malformed Params {name}");
    }
    Ok(v[0])
}
fn digest(b: &[u8]) -> String {
    hex_upper(&Sha256::digest(b))
}
fn assert_hash(
    sql: &mut String,
    table: &str,
    predicate: &str,
    column: &str,
    data: &[u8],
    extra: &str,
) {
    writeln!(sql,"IF (SELECT COUNT_BIG(*) FROM dbo.{table} WITH(UPDLOCK,HOLDLOCK) WHERE {predicate} AND DATALENGTH({column})={} AND HASHBYTES('SHA2_256',{column})=0x{} {extra})<>1 THROW 57500,N'Extension schema optimistic state changed',1;",data.len(),digest(data)).unwrap();
}

fn guard_state(s: &ExtensionRestructureState, scope: &ServiceScope) -> String {
    let mut sql = String::from(
        "DECLARE @ExtSchemaLock int; EXEC @ExtSchemaLock=sys.sp_getapplock @Resource=N'ibcmd-rs:extension-schema',@LockMode='Exclusive',@LockOwner='Transaction',@LockTimeout=0; IF @ExtSchemaLock<0 THROW 57500,N'Extension schema lock unavailable',1;\n",
    );
    writeln!(sql,"IF (SELECT COUNT_BIG(*) FROM dbo.Params WITH(UPDLOCK,HOLDLOCK) WHERE FileName LIKE N'DBNames%' OR FileName=N'a07b62f0-1f01-484a-93d9-d42764cedac0.si')<>{} THROW 57500,N'Extension names row set changed',1;",s.params.len()).unwrap();
    for p in &s.params {
        assert_hash(
            &mut sql,
            "Params",
            &format!(
                "FileName=N'{}' AND PartNo={} AND DataSize={}",
                quote_string(&p.name),
                p.part,
                p.size
            ),
            "BinaryData",
            &p.data,
            "",
        );
    }
    writeln!(sql,"IF (SELECT COUNT_BIG(*) FROM dbo.SchemaStorage WITH(UPDLOCK,HOLDLOCK))<>{} THROW 57500,N'Extension schema row set changed',1;",s.schemas.len()).unwrap();
    for r in &s.schemas {
        assert_hash(
            &mut sql,
            "SchemaStorage",
            &format!("SchemaID={} AND Status={}", r.id, r.status),
            "CurrentSchema",
            &r.current,
            &format!(
                "AND NewGenCreated=0x{} AND NewGenDropped=0x{}",
                hex_upper(&r.created),
                hex_upper(&r.dropped)
            ),
        );
    }
    writeln!(sql,"IF (SELECT COUNT_BIG(*) FROM dbo._ExtensionsInfo ctx WITH(UPDLOCK,HOLDLOCK) WHERE {})<>1 THROW 57500,N'Extension global registry context changed',1;",scope.registry_predicate()).unwrap();
    writeln!(sql,"IF (SELECT COUNT_BIG(*) FROM dbo._ExtensionsRestruct WITH(UPDLOCK,HOLDLOCK) WHERE {})<>{} THROW 57500,N'Extension scoped service row set changed',1;",scope.predicate("_ExtensionsRestruct"),s.services.len()).unwrap();
    for (id, data, int, kind) in &s.services {
        assert_hash(
            &mut sql,
            "_ExtensionsRestruct",
            &format!(
                "_ExtDataID=0x{} AND _RestructDataInt={int} AND _RestructDataType={kind} AND {}",
                hex_upper(id),
                scope.predicate("_ExtensionsRestruct")
            ),
            "_RestructData",
            data,
            "",
        );
    }
    sql
}
fn replace_param(sql: &mut String, p: &Param, after: &[u8]) {
    if p.data == after {
        return;
    }
    writeln!(sql,"UPDATE dbo.Params SET BinaryData=0x{},DataSize={},Creation=DATEADD(year,2000,SYSUTCDATETIME()),Modified=DATEADD(year,2000,SYSUTCDATETIME()) WHERE FileName=N'{}' AND PartNo=0 AND DataSize={} AND HASHBYTES('SHA2_256',BinaryData)=0x{}; IF @@ROWCOUNT<>1 THROW 57505,N'Extension names publication lost race',1;",hex_upper(after),after.len(),quote_string(&p.name),p.size,digest(&p.data)).unwrap();
}

fn reference_tables(
    s: &ExtensionRestructureState,
    objects: &[Object],
    names: &DbNames,
) -> Result<RefTables> {
    let p = param(s, "a07b62f0-1f01-484a-93d9-d42764cedac0.si")?;
    let text = inflate(&p.data).unwrap_or_else(|_| p.data.clone());
    let mut refs = RefTables::build(&NamesTables::parse(&text)?);
    for o in objects
        .iter()
        .filter(|o| matches!(o.kind.as_str(), "Catalog" | "Document" | "Enum"))
    {
        let facts = ObjectFacts::parse(&o.kind, &o.row)?;
        if let Some(n) = names.number_of(&o.uuid, o.table_kind()) {
            refs = refs.with_table(
                &o.uuid,
                &facts.generated_type("Ref")?.type_id,
                &format!("{}{n}", o.table_kind()),
            );
        }
    }
    Ok(refs)
}
fn common(root: &Path, image: &StorageImage) -> Result<CommonAttributes> {
    let doc = MetadataXml::parse(&std::fs::read(root.join("Configuration.xml"))?)?;
    let children = doc
        .object()?
        .child("ChildObjects")
        .context("configuration child list missing")?;
    let mut order = Vec::new();
    let mut descriptors = BTreeMap::new();
    let rows = bytes(image);
    for c in children.children_named("CommonAttribute") {
        let d = MetadataXml::parse(&std::fs::read(
            root.join("CommonAttributes")
                .join(format!("{}.xml", c.text.trim())),
        )?)?;
        let uuid = id(d.object()?)?;
        let packed = rows
            .get(&uuid)
            .context("common attribute descriptor missing")?;
        let raw = inflate(packed).unwrap_or_else(|_| packed.clone());
        descriptors.insert(uuid.clone(), parse_row(&raw)?);
        order.push(uuid);
    }
    CommonAttributes::from_rows(&order, &|uuid| descriptors.get(uuid).cloned())
}
fn table_entry(
    o: &Object,
    names: &DbNames,
    refs: &RefTables,
    context: &DescriptorContext,
    common: &CommonAttributes,
) -> Result<Brace> {
    if o.kind == "ScheduledJob" {
        return Ok(ibcmd_restructure::scheduled_jobs::table_entry(
            names
                .number_of(&o.uuid, JobsLayout::TABLE_KIND)
                .context("scheduled job has no DBNames entry")?,
        ));
    }
    if o.kind == "InformationRegister" {
        if !common.applying(&o.uuid).is_empty() {
            bail!(
                "{} has unmodeled extension common-attribute contributions",
                o.label()
            );
        }
        return information_register_entry(o, names, refs, context);
    }
    let facts = ObjectFacts::parse(&o.kind, &o.row)?;
    let members = Members::parse(&o.kind, &o.row)?;
    main_entry(&EntryInput {
        kind: Kind::parse(&o.kind)?,
        facts: &facts,
        members: &members,
        names,
        refs,
        common,
    })
}
fn list(items: Vec<Brace>) -> Brace {
    let mut v = vec![Brace::atom(items.len())];
    v.extend(items);
    Brace::List(v)
}
fn information_register_entry(
    o: &Object,
    names: &DbNames,
    refs: &RefTables,
    context: &DescriptorContext,
) -> Result<Brace> {
    validate_new(o)?;
    let number = names
        .number_of(&o.uuid, "InfoRg")
        .context("register has no DBNames entry")?;
    let table = format!("InfoRg{number}");
    let mut fields = Vec::new();
    let mut dims = Vec::new();
    for child in o.fields() {
        let p = child
            .child("Properties")
            .context("register member has no properties")?;
        let uuid = id(child)?;
        let field = format!(
            "Fld{}",
            names
                .number_of(&uuid, "Fld")
                .context("register field has no DBNames entry")?
        );
        let member = Member {
            uuid,
            name: p.child_text("Name").unwrap_or_default().into(),
            synonyms: vec![],
            pattern: crate::metadata_model::types::type_pattern(p.child("Type"), context)?,
            usage: None,
            indexing: Some(0),
        };
        fields.push(FieldEntry::new(&field, false, attribute_types(&member, refs)?).to_brace());
        if child.name == "Dimension" {
            dims.push(field);
        }
    }
    let index = IndexEntry {
        name: "ByDims".into(),
        unique: true,
        fields: dims,
        clustered: true,
    };
    Ok(Brace::List(vec![
        Brace::str(table),
        Brace::str("N"),
        Brace::atom(number),
        Brace::str(""),
        list(fields),
        list(vec![]),
        list(vec![index.to_brace()]),
        Brace::num(1),
        Brace::str("S"),
        list(vec![]),
        list(vec![]),
        Brace::str(""),
        Brace::num(0),
        Brace::num(0),
    ]))
}

fn allocation_steps(
    old: &[Object],
    new: &[Object],
    changed: &BTreeSet<String>,
) -> Result<Vec<NameStep>> {
    let old_ids: BTreeSet<_> = old.iter().map(|o| o.uuid.as_str()).collect();
    let created: Vec<_> = new
        .iter()
        .filter(|o| !old_ids.contains(o.uuid.as_str()))
        .collect();
    let mut steps = Vec::new();
    let alloc = |o: &Object, kind: &str| {
        NameStep::Allocate(NameRequest {
            uuid: o.uuid.clone(),
            kind: kind.into(),
        })
    };
    for o in created.iter().filter(|o| o.kind == "Catalog") {
        steps.push(alloc(o, "Reference"));
    }
    for o in created.iter().filter(|o| o.kind == "Catalog") {
        for uuid in o.field_ids()? {
            steps.push(NameStep::Allocate(NameRequest {
                uuid,
                kind: "Fld".into(),
            }));
        }
    }
    for o in created.iter().filter(|o| o.kind == "InformationRegister") {
        steps.push(alloc(o, "InfoRg"));
        for uuid in o.field_ids()? {
            steps.push(NameStep::Allocate(NameRequest {
                uuid,
                kind: "Fld".into(),
            }));
        }
    }
    for kind in ["Catalog", "ScheduledJob", "Document"] {
        for o in new.iter().filter(|o| o.kind == kind) {
            if !old_ids.contains(o.uuid.as_str()) {
                match kind {
                    "Catalog" => steps.push(NameStep::ReserveCatalog),
                    "ScheduledJob" => steps.push(alloc(o, JobsLayout::TABLE_KIND)),
                    _ => bail!("new documents are not a measured extension transition"),
                }
                continue;
            }
            if !changed.contains(&o.uuid) {
                continue;
            }
            let before = old
                .iter()
                .find(|n| n.uuid == o.uuid)
                .context("changed object missing before")?;
            let before: BTreeSet<_> = before.field_ids()?.into_iter().collect();
            for uuid in o
                .field_ids()?
                .into_iter()
                .filter(|uuid| !before.contains(uuid))
            {
                steps.push(NameStep::Allocate(NameRequest {
                    uuid,
                    kind: "Fld".into(),
                }));
            }
        }
    }
    Ok(steps)
}

// This is a separate request, not an IF around modern syntax in the same
// batch: old servers must return a precise refusal before parsing STRING_AGG
// or SQL2016 catalog columns. ProductVersion is readable on older servers.
fn with_structural_sql_capability<T>(
    database: &str,
    mut query: impl FnMut(&str) -> Result<Vec<crate::sql::SqlRow>>,
    next: impl FnOnce() -> Result<T>,
) -> Result<T> {
    fn scalar(rows: Vec<crate::sql::SqlRow>, label: &str) -> Result<crate::sql::SqlValue> {
        let [row] = rows.as_slice() else {
            bail!("extension restructuring requires one unambiguous {label} result");
        };
        if row.result_set != 0 || row.values.len() != 1 || row.value(0)?.is_null() {
            bail!("extension restructuring requires known {label}");
        }
        Ok(row.value(0)?.clone())
    }
    let version = scalar(
        query("SELECT CONVERT(nvarchar(128), SERVERPROPERTY(N'ProductVersion'))")?,
        "SQL ProductVersion",
    )?;
    let text = version
        .as_str()
        .context("extension restructuring requires a textual SQL ProductVersion")?;
    let parts = text
        .trim()
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("extension restructuring cannot parse SQL ProductVersion")?;
    if parts.len() != 4 || parts[0] < 14 {
        bail!(
            "extension restructuring requires SQL Server 2017 (major14) or later for ordered STRING_AGG and catalog checks; observed {text:?}"
        );
    }
    let compatibility = scalar(
        query(&format!(
            "SELECT CONVERT(int, compatibility_level) FROM sys.databases WHERE name=N'{}'",
            quote_string(database)
        ))?,
        "target database compatibility_level",
    )?;
    let compatibility = compatibility.as_i64().context(
        "extension restructuring requires an integer target database compatibility_level",
    )?;
    if compatibility < 110 {
        bail!(
            "extension restructuring requires target database compatibility_level >=110 for STRING_AGG WITHIN GROUP; observed {compatibility}"
        );
    }
    next()
}

/// Classify a stage before data/schema writes. `None` proves only that no
/// SQL schema/data update was planned, not that a prior source refusal is safe.
pub(crate) fn prepare(
    sql: &SqlExec,
    database: &str,
    profile: MssqlNativePlatformProfile,
    extension_id: [u8; 16],
    active: &StorageImage,
    stage: &ExtensionStagePlan,
) -> Result<Option<ExtensionRestructurePlan>> {
    if !matches!(
        profile,
        MssqlNativePlatformProfile::Platform8_3_27_2214
            | MssqlNativePlatformProfile::Platform8_5_1_1150
    ) {
        bail!(
            "extension restructuring is not measured for {}",
            profile.id()
        );
    }
    let version = if profile == MssqlNativePlatformProfile::Platform8_5_1_1150 {
        crate::cli::InfobaseConfigSourceVersion::V2_21
    } else {
        crate::cli::InfobaseConfigSourceVersion::V2_20
    };
    let proposed = crate::mssql_extension_tree_load::stage_image(active, stage)?;
    let scratch = Scratch::new()?;
    let base = scratch.0.join("before");
    let next = scratch.0.join("after");
    for (image, dir) in [(active, &base), (&proposed, &next)] {
        let r = crate::mssql_dump::extension::export_extension_image_to_source(
            image,
            dir,
            version,
            Some(crate::mssql_dump::extension::base_index_provider(
                sql, database, version,
            )),
        )?;
        if r.storage.failed != 0 || r.storage.opaque != 0 {
            bail!("extension schema update cannot classify an incomplete source export");
        }
    }
    crate::mssql_extension_tree_load::require_structural_root_correspondence(&base, &next)?;
    let old = objects(&base, active)?;
    let new = objects(&next, &proposed)?;
    let changed: BTreeSet<_> = classify(&base, &next, &old, &new)?.into_iter().collect();
    if changed.is_empty() {
        return Ok(None);
    }
    with_structural_sql_capability(
        database,
        |query| {
            sql.client()
                .context("extension restructuring requires the built-in SQL client")?
                .query_rows(query, &[])
        },
        || {
            let s = state(sql, database, extension_id)?;
            plan_from_source_trees(&base, &next, active, &proposed, extension_id, &s)
        },
    )
}

/// Plan an already exported and fully validated pair without database access.
/// This boundary is also consumed by platform-proven Coverage regressions.
pub fn plan_from_source_trees(
    base: &Path,
    next: &Path,
    active: &StorageImage,
    proposed: &StorageImage,
    extension_id: [u8; 16],
    s: &ExtensionRestructureState,
) -> Result<Option<ExtensionRestructurePlan>> {
    crate::mssql_extension_tree_load::require_structural_root_correspondence(base, next)?;
    let old = objects(base, active)?;
    let new = objects(next, proposed)?;
    let changed: BTreeSet<_> = classify(base, next, &old, &new)?.into_iter().collect();
    if changed.is_empty() {
        return Ok(None);
    }
    if s.schemas
        .iter()
        .any(|r| r.status != 100 || r.created != EMPTY_GENERATION || r.dropped != EMPTY_GENERATION)
    {
        bail!("extension schema update requires idle schema generations");
    }
    let schema = s
        .schemas
        .iter()
        .find(|r| r.id == 1)
        .context("extension schema row 1 is absent")?;
    let before_schema = DbSchema::parse(&schema.current)?;
    let target_name = format!("DBNames-Ext-{}", extension_namespace_prefix(extension_id));
    let system_name = "DBNames-Ext-1";
    let target = param(&s, &target_name)?;
    let system = param(&s, system_name)?;
    let names = DbNames::parse_row(&target.data)?;
    let system_names = DbNames::parse_row(&system.data)?;
    let all = s
        .params
        .iter()
        .filter(|p| p.name == "DBNames" || p.name.starts_with("DBNames-Ext-"))
        .map(|p| DbNames::parse_row(&p.data).map(|n| n.max))
        .collect::<Result<Vec<_>>>()?;
    let steps = allocation_steps(&old, &new, &changed)?;
    let (new_names, new_system) = allocate_extension_names(&names, &system_names, &all, &steps)?;
    let refs = reference_tables(&s, &new, &new_names)?;
    let old_context = DescriptorContext::new(
        base,
        MetadataXml::parse(&std::fs::read(base.join("Configuration.xml"))?)?
            .version()
            .unwrap_or("2.20"),
    )?;
    let new_context = DescriptorContext::new(
        next,
        MetadataXml::parse(&std::fs::read(next.join("Configuration.xml"))?)?
            .version()
            .unwrap_or("2.20"),
    )?;
    let common = common(base, active)?;
    let mut changes = Vec::new();
    let mut deferred = Vec::new();
    let mut job_changes = Vec::new();
    let mut enums = Vec::new();
    let mut labels = Vec::new();
    for n in new.iter().filter(|n| changed.contains(&n.uuid)) {
        labels.push(n.label());
        let o = old.iter().find(|o| o.uuid == n.uuid);
        if n.kind == "ScheduledJob" {
            if o.is_none() {
                job_changes.push(TableChange {
                    before: None,
                    after: table_entry(n, &new_names, &refs, &new_context, &common)?,
                });
            }
            continue;
        }
        if n.kind == "Enum" {
            let o = o.context("new enums are not measured")?;
            enums.push((o, n));
            continue;
        }
        if let Some(o) = o {
            if o.field_ids()? == n.field_ids()? {
                continue;
            }
            let before = table_entry(o, &names, &refs, &old_context, &common)?;
            let name = TableView::new(&before)?.name().to_owned();
            let stored = before_schema.tables()[before_schema
                .position(&name)
                .with_context(|| format!("stored extension table {name} missing"))?]
            .clone();
            if stored != before {
                bail!(
                    "stored {} schema does not match the typed metadata model",
                    o.label()
                );
            }
            deferred.push(TableChange {
                before: Some(stored),
                after: table_entry(n, &new_names, &refs, &new_context, &common)?,
            });
        } else {
            changes.push(TableChange {
                before: None,
                after: table_entry(n, &new_names, &refs, &new_context, &common)?,
            });
        }
    }
    if changes.is_empty() && deferred.is_empty() && enums.is_empty() && job_changes.is_empty() {
        return Ok(None);
    }
    // The platform moves this unchanged system definition before rebuilt own
    // objects; preserving its rows avoids changing unrelated history data.
    if let Some(i) = before_schema.position("DataHistoryMetadataExt") {
        let entry = before_schema.tables()[i].clone();
        changes.push(TableChange {
            before: Some(entry.clone()),
            after: entry,
        });
    }
    changes.extend(deferred);
    changes.extend(job_changes);
    for (_, n) in &enums {
        let name = format!(
            "Enum{}",
            names
                .number_of(&n.uuid, "Enum")
                .context("enum table missing")?
        );
        let i = before_schema
            .position(&name)
            .context("enum schema entry missing")?;
        let e = before_schema.tables()[i].clone();
        changes.push(TableChange {
            before: Some(e.clone()),
            after: e,
        });
    }
    let plan = SchemaPlan::build(&schema.current, &changes)?;
    let scope = ServiceScope::from_schemas(&s.schemas, extension_id)?;
    let mut phase = guard_state(&s, &scope);
    for (_, n) in &enums {
        let logical = format!(
            "Enum{}",
            names
                .number_of(&n.uuid, "Enum")
                .context("enum table missing")?
        );
        for physical in
            crate::restructure::schema::physical_tables(&before_schema.named(&logical)?)?
        {
            ibcmd_restructure::extension::assert_structure(&mut phase, &physical, "X1");
            writeln!(phase,"IF EXISTS(SELECT 1 FROM sys.triggers WHERE parent_id=OBJECT_ID(N'dbo.{}X1')) THROW 57507,N'Unmodeled trigger on extension enum table',1;",physical.name).unwrap();
        }
    }
    phase.push_str(&plan.ddl()?);
    let mut tables = plan
        .tables
        .iter()
        .map(|t| format!("{}X1", t.after.name))
        .collect::<Vec<_>>();
    for (o, n) in enums {
        let table = format!(
            "_Enum{}X1",
            names
                .number_of(&n.uuid, "Enum")
                .context("enum table missing")?
        );
        let old_values: Vec<_> = o
            .children()
            .iter()
            .filter(|c| c.name == "EnumValue")
            .collect();
        let new_values: Vec<_> = n
            .children()
            .iter()
            .filter(|c| c.name == "EnumValue")
            .collect();
        for (order, c) in new_values.iter().enumerate().skip(old_values.len()) {
            let uuid = uuid::Uuid::parse_str(&id(c)?)?;
            let b = uuid.as_bytes();
            let physical = [&b[8..16], &b[6..8], &b[4..6], &b[0..4]].concat();
            writeln!(phase,"IF EXISTS(SELECT 1 FROM dbo.{table} WITH(UPDLOCK,HOLDLOCK) WHERE _IDRRef=0x{} OR _EnumOrder={order}) THROW 57506,N'Enum value identity already exists',1; INSERT dbo.{table}(_IDRRef,_EnumOrder) VALUES(0x{},{order});",hex_upper(&physical),hex_upper(&physical)).unwrap();
        }
        if !tables.contains(&table) {
            tables.push(table);
        }
    }
    let mut after_scheduled_jobs = None;
    if new
        .iter()
        .any(|o| o.kind == "ScheduledJob" && changed.contains(&o.uuid))
    {
        let id = uuid::Uuid::parse_str(JobsLayout::SERVICE_ID)?.to_bytes_le();
        let existing = s
            .services
            .iter()
            .find(|(key, _, _, _)| key.as_slice() == id);
        let before = if let Some((_, data, int, kind)) = existing {
            if *int != 0 || *kind != 0 {
                bail!("unmodeled scheduled-job service numeric flags");
            }
            JobService::parse(data)?
        } else {
            JobService::default()
        };
        let old_jobs = old
            .iter()
            .filter(|o| o.kind == "ScheduledJob")
            .map(|o| {
                Ok((
                    o.uuid.clone(),
                    format!(
                        "{}{}",
                        JobsLayout::TABLE_KIND,
                        names
                            .number_of(&o.uuid, JobsLayout::TABLE_KIND)
                            .context("old scheduled job has no DBNames entry")?
                    ),
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mapped = before
            .groups
            .iter()
            .find(|g| {
                g.extension
                    .eq_ignore_ascii_case(&extension_namespace_prefix(extension_id))
            })
            .map(|g| {
                g.jobs
                    .iter()
                    .map(|j| (j.uuid.clone(), j.table.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        if old_jobs != mapped {
            bail!(
                "active scheduled-job service mapping disagrees with extension metadata and names"
            );
        }
        let jobs = new
            .iter()
            .filter(|o| o.kind == "ScheduledJob")
            .map(|o| {
                Ok(ScheduledJob {
                    uuid: o.uuid.clone(),
                    table: format!(
                        "{}{}",
                        JobsLayout::TABLE_KIND,
                        new_names
                            .number_of(&o.uuid, JobsLayout::TABLE_KIND)
                            .context("scheduled job has no DBNames entry")?
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let row = before
            .with_group(&extension_namespace_prefix(extension_id), jobs)?
            .to_bytes();
        after_scheduled_jobs = Some(row.clone());
        if let Some((_, old, _, _)) = existing {
            writeln!(phase,"UPDATE dbo._ExtensionsRestruct SET _RestructData=0x{} WHERE _ExtDataID=0x{} AND _RestructDataInt=0 AND _RestructDataType=0 AND DATALENGTH(_RestructData)={} AND HASHBYTES('SHA2_256',_RestructData)=0x{} AND {}; IF @@ROWCOUNT<>1 THROW 57505,N'Scheduled-job mapping changed before publication',1;",hex_upper(&row),hex_upper(&id),old.len(),digest(old),scope.predicate("_ExtensionsRestruct")).unwrap();
        } else {
            writeln!(phase,"IF EXISTS(SELECT 1 FROM dbo._ExtensionsRestruct WITH(UPDLOCK,HOLDLOCK) WHERE _ExtDataID=0x{} AND {}) THROW 57505,N'Scheduled-job mapping appeared after planning',1; INSERT dbo._ExtensionsRestruct(_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType{}) SELECT 0x{},0x{},0,0{} FROM dbo._ExtensionsInfo ctx WITH(UPDLOCK,HOLDLOCK) WHERE {}; IF @@ROWCOUNT<>1 THROW 57505,N'Scheduled-job context copy lost race',1;",hex_upper(&id),scope.predicate("_ExtensionsRestruct"),scope.insert_columns(),hex_upper(&id),hex_upper(&row),scope.insert_values(),scope.registry_predicate()).unwrap();
        }
    }
    let after_names = new_names.to_row()?;
    let after_system_names = new_system.to_row()?;
    if target.data != after_names || system.data != after_system_names {
        tables.push("Params".to_owned());
    }
    replace_param(&mut phase, target, &after_names);
    replace_param(&mut phase, system, &after_system_names);
    for name in [&target_name, system_name] {
        let old = param(&s, &format!("DBNamesVersion-{name}"))?;
        let changed = if name == system_name {
            new_system != system_names
        } else {
            new_names != names
        };
        if changed {
            replace_param(
                &mut phase,
                old,
                &version_row(&uuid::Uuid::new_v4().to_string()),
            );
        }
    }
    writeln!(phase,"UPDATE dbo.SchemaStorage SET CurrentSchema=0x{},Status=100,NewGenCreated=0x{},NewGenDropped=0x{} WHERE SchemaID=1 AND Status=100 AND HASHBYTES('SHA2_256',CurrentSchema)=0x{}; IF @@ROWCOUNT<>1 THROW 57505,N'Extension schema publication lost race',1;",hex_upper(&plan.after),hex_upper(EMPTY_GENERATION),hex_upper(EMPTY_GENERATION),digest(&schema.current)).unwrap();
    assert_hash(
        &mut phase,
        "SchemaStorage",
        "SchemaID=1 AND Status=100",
        "CurrentSchema",
        &plan.after,
        "",
    );
    tables.push("SchemaStorage".to_owned());
    if after_scheduled_jobs.is_some() {
        tables.push("_ExtensionsRestruct".to_owned());
    }
    Ok(Some(ExtensionRestructurePlan {
        sql: phase,
        objects: labels,
        tables,
        after_schema: plan.after,
        after_names,
        after_system_names,
        after_scheduled_jobs,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn enumeration(values: &[&str]) -> Object {
        let mut xml = String::from(
            r#"<MetaDataObject><Enum uuid="aaaaaaaa-0000-4000-8000-000000000001"><Properties><Name>E</Name><DataHistory>DontUse</DataHistory></Properties><ChildObjects>"#,
        );
        for name in values {
            let guid = match *name {
                "A" => "aaaaaaaa-0000-4000-8000-000000000002",
                "B" => "aaaaaaaa-0000-4000-8000-000000000003",
                _ => "aaaaaaaa-0000-4000-8000-000000000004",
            };
            write!(xml,r#"<EnumValue uuid="{guid}"><Properties><Name>{name}</Name></Properties></EnumValue>"#).unwrap();
        }
        xml.push_str("</ChildObjects></Enum></MetaDataObject>");
        let parsed = MetadataXml::parse(xml.as_bytes()).unwrap();
        Object {
            family: "Enums".into(),
            kind: "Enum".into(),
            uuid: "aaaaaaaa-0000-4000-8000-000000000001".into(),
            name: "E".into(),
            element: parsed.object().unwrap().clone(),
            row: Brace::List(vec![]),
        }
    }
    fn context_schema(extra: Vec<FieldEntry>) -> Vec<Schema> {
        let f = |name: &str, tag: &str, a: u64| {
            FieldEntry::new(
                name,
                false,
                vec![crate::restructure::schema::TypeEntry::new(tag, a, 0, "", 0)],
            )
        };
        let table = |name: &str, mut fields: Vec<FieldEntry>| {
            fields.extend(extra.clone());
            Brace::List(vec![
                Brace::str(name),
                Brace::str("N"),
                Brace::num(1),
                Brace::str(""),
                list(fields.into_iter().map(|f| f.to_brace()).collect()),
                list(vec![]),
                list(vec![]),
                Brace::num(1),
                Brace::str("S"),
                list(vec![]),
                list(vec![]),
                Brace::str(""),
                Brace::num(0),
                Brace::num(0),
            ])
        };
        let service = table(
            "ExtensionsRestruct",
            vec![
                f("ExtDataID", "B", 16),
                f("RestructData", "B", 0x80000000),
                f("RestructDataInt", "N", 9),
                f("RestructDataType", "N", 9),
            ],
        );
        let registry = table(
            "ExtensionsInfo",
            vec![f("ID", "B", 16), f("ExtensionScope", "N", 2)],
        );
        vec![Schema {
            id: 0,
            status: 100,
            current: DbSchema::from_tables(vec![service, registry]).to_text(),
            created: EMPTY_GENERATION.to_vec(),
            dropped: EMPTY_GENERATION.to_vec(),
        }]
    }
    #[test]
    fn final_admission_sql_capability_accepts_evidenced_floor_and_denies_ambiguous_rows() {
        use crate::sql::{SqlRow, SqlValue};
        for (version, level) in [
            ("14.0.1000.169", 110),
            ("17.0.1135.8", 170),
            ("18.0.1.2", 110),
        ] {
            let mut queries = Vec::new();
            let result = with_structural_sql_capability(
                "target'quoted",
                |query| {
                    queries.push(query.to_owned());
                    let value = if queries.len() == 1 {
                        assert_eq!(
                            query,
                            "SELECT CONVERT(nvarchar(128), SERVERPROPERTY(N'ProductVersion'))"
                        );
                        SqlValue::Text(version.into())
                    } else {
                        assert!(query.contains("name=N'target''quoted'"));
                        SqlValue::Int(level)
                    };
                    Ok(vec![SqlRow {
                        result_set: 0,
                        values: vec![value],
                    }])
                },
                || Ok("planned"),
            )
            .unwrap();
            assert_eq!(result, "planned");
            assert_eq!(queries.len(), 2);
        }
        for rows in [
            vec![],
            vec![SqlRow {
                result_set: 0,
                values: vec![SqlValue::Int(17)],
            }],
            vec![
                SqlRow {
                    result_set: 0,
                    values: vec![SqlValue::Text("17.0.1.2".into())]
                };
                2
            ],
            vec![SqlRow {
                result_set: 0,
                values: vec![SqlValue::Text("17.0.1.2".into()), SqlValue::Int(1)],
            }],
        ] {
            assert!(
                with_structural_sql_capability::<()>(
                    "db",
                    |_| Ok(rows.clone()),
                    || panic!("ambiguous capability reached state/plan")
                )
                .is_err()
            );
        }
        for rows in [
            vec![],
            vec![SqlRow {
                result_set: 0,
                values: vec![SqlValue::Text("110".into())],
            }],
            vec![
                SqlRow {
                    result_set: 0,
                    values: vec![SqlValue::Int(110)]
                };
                2
            ],
        ] {
            let mut reads = 0;
            assert!(
                with_structural_sql_capability::<()>(
                    "db",
                    |_| {
                        reads += 1;
                        if reads == 1 {
                            Ok(vec![SqlRow {
                                result_set: 0,
                                values: vec![SqlValue::Text("17.0.1135.8".into())],
                            }])
                        } else {
                            Ok(rows.clone())
                        }
                    },
                    || panic!("missing/ambiguous/wrong-type compatibility reached state/plan")
                )
                .is_err()
            );
            assert_eq!(reads, 2);
        }
    }

    #[test]
    fn final_admission_sql_capability_refuses_before_catalog_or_plan_queries() {
        use crate::sql::{SqlRow, SqlValue};
        for (version, compatibility) in [
            (Some("13.0.6300.2"), Some(130)),
            (Some("10.50.6000.34"), Some(100)),
            (None, Some(150)),
            (Some("unknown"), Some(150)),
            (Some("14.0.1000.169"), Some(100)),
            (Some("17.0.1135.8"), None),
        ] {
            let entered = std::cell::Cell::new(false);
            let mut queries = Vec::new();
            let result = with_structural_sql_capability(
                "target",
                |query| {
                    queries.push(query.to_owned());
                    assert!(
                        !query.contains("TRY_CONVERT")
                            && !query.contains("STRING_AGG")
                            && !query.contains("sys.columns")
                    );
                    let value = if query.contains("ProductVersion") {
                        version
                            .map(|v| SqlValue::Text(v.into()))
                            .unwrap_or(SqlValue::Null)
                    } else {
                        compatibility.map(SqlValue::Int).unwrap_or(SqlValue::Null)
                    };
                    Ok(vec![SqlRow {
                        result_set: 0,
                        values: vec![value],
                    }])
                },
                || {
                    entered.set(true);
                    Ok(())
                },
            );
            assert!(
                result.is_err(),
                "unsupported/unknown capability must refuse: {version:?}/{compatibility:?}"
            );
            assert!(
                !entered.get(),
                "no state, catalog, DDL plan or publication may follow refusal"
            );
        }
    }

    #[test]
    fn final_admission_structural_plan_preserves_retained_tableless_identity() {
        use ibcmd_cf::{archive::decode_archive_uniform, payload::PayloadEncoding};
        use ibcmd_core::{
            artifact::StorageProfileId, limits::ResourceLimits, storage::StorageProvenance,
        };
        let set = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/1c-fixtures/coverage/8.5.1.1150/extensions/restructuring");
        let base = set.join("native/CovRestructureBaseline");
        let native_update = set.join("native/CovRestructureUpdate");
        let scratch = Scratch::new().unwrap();
        let next = scratch.0.join("after");
        for relative in crate::load::relative_files(&native_update).unwrap() {
            let target = next.join(&relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(native_update.join(&relative), target).unwrap();
        }
        let decode = |name: &str| {
            let packed = crate::module_blob::decode_base64_mime(
                &std::fs::read_to_string(set.join(format!("variants/{name}.cfe.b64"))).unwrap(),
            )
            .unwrap();
            decode_archive_uniform(
                std::io::Cursor::new(&packed),
                ResourceLimits::for_input_bytes(packed.len() as u64),
                StorageProfileId::parse("mssql-extension-8.3.27").unwrap(),
                StorageProvenance::new("public native Coverage combined admission regression")
                    .unwrap(),
                PayloadEncoding::RawDeflate,
            )
            .unwrap()
        };
        let active = decode("CovRestructureBaseline");
        let proposed = decode("CovRestructureUpdate");
        let old = objects(&base, active.image()).unwrap();
        let new = objects(&next, proposed.image()).unwrap();
        let changed = classify(&base, &next, &old, &new).unwrap();
        assert!(new.iter().any(|o| o.kind == "Catalog"
            && !old.iter().any(|before| before.uuid == o.uuid)
            && changed.contains(&o.uuid)));
        let state = ExtensionRestructureState {
            params: vec![],
            schemas: vec![],
            services: vec![],
        };
        let control = plan_from_source_trees(
            &base,
            &next,
            active.image(),
            proposed.image(),
            [0; 16],
            &state,
        )
        .unwrap_err();
        assert!(
            format!("{control:#}").contains("extension schema row 1 is absent"),
            "valid native catalog additions must reach schema-state validation: {control:#}"
        );
        let form = next.join("CommonForms/Cov_ActivationForm.xml");
        let xml = std::fs::read_to_string(&form).unwrap();
        let model = MetadataXml::parse(xml.as_bytes()).unwrap();
        let old_id = model.object().unwrap().attr("uuid").unwrap();
        let changed = xml.replacen(
            &format!("uuid=\"{old_id}\""),
            "uuid=\"aaaaaaaa-0000-4000-8000-000000000099\"",
            1,
        );
        assert_ne!(changed, xml);
        std::fs::write(&form, changed).unwrap();
        let refusal = plan_from_source_trees(
            &base,
            &next,
            active.image(),
            proposed.image(),
            [0; 16],
            &state,
        )
        .unwrap_err();
        assert!(
            format!("{refusal:#}").contains("changes descriptor uuid"),
            "a valid schema addition must not override retained tableless identity refusal: {refusal:#}"
        );
        std::fs::write(&form, &xml).unwrap();
        let module = "Catalogs/Cov_Own/Ext/ObjectModule.bsl";
        assert!(base.join(module).is_file() && next.join(module).is_file());
        std::fs::remove_file(next.join(module)).unwrap();
        let refusal = plan_from_source_trees(
            &base,
            &next,
            active.image(),
            proposed.image(),
            [0; 16],
            &state,
        )
        .unwrap_err();
        assert!(
            format!("{refusal:#}").contains("source-transition refusal")
                && format!("{refusal:#}").contains(module),
            "a valid schema addition must not override an unmodeled retained-body refusal: {refusal:#}"
        );
    }
    #[test]
    fn final_admission_skipped_assets_preserve_source_rules_and_form_module_exception() {
        for (path, admitted) in [
            ("Catalogs/Existing/Ext/ObjectModule.bsl", false),
            ("Catalogs/Existing/Forms/Flow.xml", false),
            ("Catalogs/Existing/Templates/Print.xml", false),
            ("Ext/SessionModule.bsl", false),
            ("DataProcessors/Existing/Other/Unknown.xml", false),
            ("DataProcessors/Existing/Ext/ObjectModule.bsl", true),
            ("DataProcessors/Existing/Forms/Flow.xml", true),
            ("Catalogs/Existing/Forms/Flow/Ext/Form/Module.bsl", true),
        ] {
            for added in [false, true] {
                let base = Scratch::new().unwrap();
                let next = Scratch::new().unwrap();
                let target = if added { &next.0 } else { &base.0 }.join(path);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, b"source asset").unwrap();
                assert_eq!(
                    classify(&base.0, &next.0, &[], &[]).is_ok(),
                    admitted,
                    "{path}, added={added}"
                );
            }
        }
    }
    #[test]
    fn service_context_is_copied_and_guarded_without_nullable_default_guesses() {
        let fields = vec![
            FieldEntry::new(
                "DataSeparationUse901",
                false,
                vec![crate::restructure::schema::TypeEntry::new("L", 0, 0, "", 0)],
            ),
            FieldEntry::new(
                "Fld902",
                true,
                vec![crate::restructure::schema::TypeEntry::new(
                    "S", 0x8000000a, 0, "", 0,
                )],
            ),
        ];
        let scope = ServiceScope::from_schemas(&context_schema(fields), [1; 16]).unwrap();
        assert_eq!(scope.insert_columns(), ",[_DataSeparationUse901],[_Fld902]");
        assert_eq!(
            scope.insert_values(),
            ",ctx.[_DataSeparationUse901],ctx.[_Fld902]"
        );
        let predicate = scope.predicate("r");
        assert!(predicate.contains("ctx._ExtensionScope=1"));
        assert!(predicate.contains("ctx.[_DataSeparationUse901]=0x00"));
        assert!(predicate.contains(
            "ctx.[_Fld902]=r.[_Fld902] OR (ctx.[_Fld902] IS NULL AND r.[_Fld902] IS NULL)"
        ));
        assert!(ServiceScope::from_schemas(&[], [1; 16]).is_err());
        let bad = vec![FieldEntry::new(
            "UnknownScope",
            true,
            vec![crate::restructure::schema::TypeEntry::new(
                "S", 10, 0, "", 0,
            )],
        )];
        assert!(ServiceScope::from_schemas(&context_schema(bad), [1; 16]).is_err());
    }

    #[test]
    fn predefined_job_instances_refuse_before_schema_planning() {
        let mut job = enumeration(&[]);
        job.kind = "ScheduledJob".into();
        job.family = "ScheduledJobs".into();
        job.element.name = "ScheduledJob".into();
        let p = job
            .element
            .children
            .iter_mut()
            .find(|c| c.name == "Properties")
            .unwrap();
        p.children.push(Element {
            name: "Predefined".into(),
            text: "true".into(),
            ..Element::default()
        });
        assert!(
            validate_new(&job)
                .unwrap_err()
                .to_string()
                .contains("predefined scheduled-job")
        );
        let p = job
            .element
            .children
            .iter_mut()
            .find(|c| c.name == "Properties")
            .unwrap();
        p.children
            .iter_mut()
            .find(|c| c.name == "Predefined")
            .unwrap()
            .text = "false".into();
        assert!(validate_new(&job).is_ok());
    }

    #[test]
    fn retained_paths_cannot_replace_old_data_object_identities() {
        let before = enumeration(&["A"]);
        let mut after = before.clone();
        after.uuid = "aaaaaaaa-0000-4000-8000-000000000099".into();
        assert!(validate_identities(&[before.clone()], &[after]).is_err());
        let mut renamed = before.clone();
        renamed.name = "Renamed".into();
        assert!(validate_identities(&[before.clone()], &[renamed]).is_err());
        assert!(validate_identities(&[before.clone()], &[before]).is_ok());
    }

    #[test]
    fn enum_values_can_only_append_without_reordering_or_replacing_existing_ids() {
        let old = enumeration(&["A", "B"]);
        assert!(validate_object_update(&old, &enumeration(&["A", "B", "C"])).is_ok());
        for changed in [vec!["A", "C", "B"], vec!["B", "A", "C"], vec!["A", "C"]] {
            assert!(
                validate_object_update(&old, &enumeration(&changed)).is_err(),
                "{changed:?}"
            );
        }
    }
    #[test]
    fn data_processor_dialog_paths_do_not_request_sql_schema_planning() {
        let base = Scratch::new().unwrap();
        let next = Scratch::new().unwrap();
        for root in [&base.0, &next.0] {
            std::fs::create_dir_all(root.join("DataProcessors/Tools/Forms/Import/Ext")).unwrap();
        }
        std::fs::write(base.0.join("DataProcessors/Tools.xml"), b"before").unwrap();
        std::fs::write(next.0.join("DataProcessors/Tools.xml"), b"after").unwrap();
        std::fs::write(
            next.0.join("DataProcessors/Tools/Forms/Import.xml"),
            b"form descriptor",
        )
        .unwrap();
        std::fs::write(
            next.0
                .join("DataProcessors/Tools/Forms/Import/Ext/Form.xml"),
            b"form body",
        )
        .unwrap();
        assert!(classify(&base.0, &next.0, &[], &[]).unwrap().is_empty());
        std::fs::create_dir_all(next.0.join("Unmodeled")).unwrap();
        std::fs::write(next.0.join("Unmodeled/Object.xml"), b"unknown").unwrap();
        assert!(classify(&base.0, &next.0, &[], &[]).is_err());
    }
}
