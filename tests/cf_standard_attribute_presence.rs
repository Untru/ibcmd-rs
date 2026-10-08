//! Portable export acceptance for absent native standard-attribute collections.
//! All records are authored from the declared layout, without a foreign CF.

use flate2::{Compression, write::DeflateEncoder};
use ibcmd_rs::{cli::InfobaseConfigSourceVersion, mssql_dump::export_packed_entries_to_source};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn uuid(seed: usize) -> String {
    format!("00000000-0000-4000-8000-{seed:012x}")
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-standard-presence-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn packed(text: &str) -> Vec<u8> {
    let mut writer = DeflateEncoder::new(Vec::new(), Compression::default());
    writer.write_all(text.as_bytes()).unwrap();
    writer.finish().unwrap()
}

fn task() -> String {
    let nil = "00000000-0000-0000-0000-000000000000";
    let mut fields = vec!["0".to_owned(); 52];
    fields[0] = "33".into();
    fields[1] = format!(
        "{{3,{{1,0,{}}},\"PresenceTask\",{{0}},\"\",0,0,{nil},0}}",
        uuid(100)
    );
    fields[2] = "1".into();
    for (offset, field) in fields[3..13].iter_mut().enumerate() {
        *field = uuid(110 + offset);
    }
    for slot in [13, 14, 15, 16, 17, 25, 26, 29, 35, 36, 37] {
        fields[slot] = nil.into();
    }
    fields[18] = "1".into();
    fields[19] = "9".into();
    fields[20] = "1".into();
    fields[21] = "1".into();
    fields[22] = "100".into();
    fields[23] = "1".into();
    fields[27] = "1".into();
    fields[28] = "{1,{0,0}}".into();
    fields[30] = "{0,0}".into();
    fields[34] = "{0}".into();
    for field in &mut fields[38..43] {
        *field = "{0}".into();
    }
    fields[44] = "{0,{0}}".into();
    fields[45] = "1".into();
    fields[46] = "{1,{0,0}}".into();
    fields[47] = "{1,2,0}".into();
    let collections = (0..6)
        .map(|offset| format!("{{{},0}}", uuid(130 + offset)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{1,{{{}}},6,{collections}}}", fields.join(","))
}

fn accounting(collection: &str) -> String {
    let nil = "00000000-0000-0000-0000-000000000000";
    let types = [nil; 14].join(",");
    format!(
        "{{1,{{21,{types},{{0,{{3,{{1,0,{}}},\"Ledger\",{{0}},\"\",0,0,{nil},0}}}},1,1,{nil},{nil},0,0,0,1,{collection}}}}}",
        uuid(200)
    )
}

fn export(collection: &str, profile: InfobaseConfigSourceVersion) -> (Scratch, String, String) {
    let scratch = Scratch::new();
    let report = export_packed_entries_to_source(
        "platform-8.3.27.1989",
        vec![
            (uuid(100), packed(&task())),
            (uuid(200), packed(&accounting(collection))),
        ],
        &scratch.0,
        true,
        profile,
        None,
    )
    .expect("both complete cleanroom metadata records must export");
    let report = serde_json::to_value(report).unwrap();
    assert_eq!(report["storage"]["failed"], 0, "{report}");
    assert_eq!(report["storage"]["opaque"], 0, "{report}");
    let task = fs::read_to_string(scratch.0.join("Tasks/PresenceTask.xml")).unwrap();
    let register = fs::read_to_string(scratch.0.join("AccountingRegisters/Ledger.xml")).unwrap();
    (scratch, task, register)
}

#[test]
fn cf_export_preserves_absent_task_and_accounting_standard_attributes_both_profiles() {
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let (_scratch, task, register) = export("{0}", profile);
        for xml in [&task, &register] {
            assert!(!xml.contains("<StandardAttributes>"), "{xml}");
            assert!(!xml.contains("<xr:StandardAttribute "), "{xml}");
        }
        assert!(task.contains("<NumberLength>9</NumberLength>"));
        assert!(register.contains("<Name>Ledger</Name>"));
    }
}

#[test]
fn cf_export_does_not_conflate_present_empty_accounting_collection_with_absence() {
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let (_scratch, task, register) = export("{1,{1,0}}", profile);
        assert!(!task.contains("<StandardAttributes>"));
        assert_eq!(register.matches("<StandardAttributes>").count(), 1);
        assert_eq!(register.matches("<xr:StandardAttribute name=").count(), 11);
        assert!(register.contains("name=\"Account\""));
        assert!(!register.contains("name=\"RecordType\""));
    }
}
