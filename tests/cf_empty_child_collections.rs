//! Public packed-storage export controls for empty native child collections.
//! Descriptors and names are generated cleanroom inputs, not a foreign CF.
use flate2::{Compression, write::DeflateEncoder};
use ibcmd_rs::{
    cli::InfobaseConfigSourceVersion,
    metadata_model::{
        brace::serialize_row,
        common::{Header, HttpService, IntegrationService},
    },
    mssql_dump::export_packed_entries_to_source,
};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn uuid(n: usize) -> String {
    format!("40000000-0000-4000-8000-{n:012x}")
}
fn header(n: usize, name: &str) -> Header {
    Header {
        uuid: uuid(n),
        name: name.into(),
        ..Header::default()
    }
}
fn packed(bytes: &[u8]) -> Vec<u8> {
    let mut w = DeflateEncoder::new(Vec::new(), Compression::default());
    w.write_all(bytes).unwrap();
    w.finish().unwrap()
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!(
            "ibcmd-empty-children-{}-{n}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn journal() -> Vec<u8> {
    let nil = "00000000-0000-0000-0000-000000000000";
    let h = header(3, "EmptyJournal");
    let ids = (10..16).map(uuid).collect::<Vec<_>>();
    let collections = [
        "3daea016-69b7-4ed4-9453-127911372fe6",
        "5aee69df-0513-4c6c-9815-103102471712",
        "a49a35ce-120a-4c80-8eea-b0618479cd70",
        "ec81ad10-ca07-11d5-b9a5-0050bae0a95d",
    ]
    .map(|id| format!("{{{id},0}}"));
    format!("{{1,{{26,{},{},{{0,{{3,{{1,0,{}}},\"{}\",{{0}},\"\",0,0,{nil},0}}}},{nil},1,{{0,0}},0,{},{},{},{},{{0}},{nil},{{0}},{{0}},{{0}}}},4,{},{},{},{}}}",ids[2],ids[3],h.uuid,h.name,ids[4],ids[5],ids[0],ids[1],collections[0],collections[1],collections[2],collections[3]).into_bytes()
}

#[test]
fn empty_service_and_journal_children_survive_public_cf_export_both_profiles() {
    let http = HttpService {
        header: header(1, "EmptyHTTP"),
        root_url: "example".into(),
        reuse_sessions: 2,
        session_max_age: 20,
        templates: vec![],
    };
    let integration = IntegrationService {
        header: header(2, "EmptyIntegration"),
        manager_type_id: uuid(20),
        manager_value_id: uuid(21),
        external_address: String::new(),
        channels: vec![],
    };
    for version in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let scratch = Scratch::new();
        let rows = vec![
            (uuid(1), packed(&serialize_row(&http.to_brace()))),
            (uuid(2), packed(&serialize_row(&integration.to_brace()))),
            (uuid(3), packed(&journal())),
        ];
        let out = scratch.0.join("export");
        let report = export_packed_entries_to_source(
            "storage:cleanroom-empty-children",
            rows,
            &out,
            false,
            version,
            None,
        )
        .unwrap();
        assert_eq!(report.storage.supported, 3, "{report:?}");
        assert_eq!(report.storage.opaque, 0, "{report:?}");
        assert_eq!(report.storage.failed, 0, "{report:?}");
        for (folder, kind, name) in [
            ("HTTPServices", "HTTPService", "EmptyHTTP"),
            (
                "IntegrationServices",
                "IntegrationService",
                "EmptyIntegration",
            ),
            ("DocumentJournals", "DocumentJournal", "EmptyJournal"),
        ] {
            let text = fs::read_to_string(out.join(folder).join(format!("{name}.xml"))).unwrap();
            assert_eq!(text.matches("<ChildObjects").count(), 1, "{kind}: {text}");
            assert!(
                text.contains(&format!(
                    "\t\t</Properties>\r\n\t\t<ChildObjects/>\r\n\t</{kind}>"
                )),
                "{text}"
            );
            let parsed =
                ibcmd_rs::metadata_model::xml::MetadataXml::parse(text.as_bytes()).unwrap();
            let object = parsed.object().unwrap();
            assert_eq!(object.name, kind);
            assert!(object.child("ChildObjects").unwrap().children.is_empty());
        }
    }
}
