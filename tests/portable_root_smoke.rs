use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ibcmd_rs::compatibility::current_compatibility_report;
use ibcmd_rs::source::{SourceKind, scan_sources};

struct TempSourceTree(PathBuf);

impl TempSourceTree {
    fn create() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-portable-smoke-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("CommonModules/Portable/Ext"))
            .expect("create portable source tree");
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempSourceTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn scans_minimal_source_tree_without_external_tools() {
    let tree = TempSourceTree::create();
    fs::write(
        tree.path().join("Configuration.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Configuration uuid="11111111-1111-4111-8111-111111111111"/>
</MetaDataObject>
"#,
    )
    .expect("write Configuration.xml");
    fs::write(
        tree.path().join("CommonModules/Portable.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <CommonModule uuid="22222222-2222-4222-8222-222222222222">
    <Properties><Name>Portable</Name></Properties>
  </CommonModule>
</MetaDataObject>
"#,
    )
    .expect("write metadata XML");
    fs::write(
        tree.path().join("CommonModules/Portable/Ext/Module.bsl"),
        "Procedure PortableSmoke()\nEndProcedure\n",
    )
    .expect("write module text");

    let manifest = scan_sources(tree.path()).expect("scan portable source tree");
    let files = manifest
        .files
        .iter()
        .map(|file| (file.path.as_str(), &file.kind))
        .collect::<Vec<_>>();

    assert_eq!(
        files,
        vec![
            ("CommonModules/Portable.xml", &SourceKind::MetadataXml),
            ("CommonModules/Portable/Ext/Module.bsl", &SourceKind::Module,),
            ("Configuration.xml", &SourceKind::ConfigurationRoot),
        ]
    );
}

#[test]
fn compatibility_report_exposes_platform_free_conversion() {
    let report = current_compatibility_report().unwrap();

    assert!(
        report
            .routes
            .iter()
            .any(|route| route.operation == "convert")
    );
    assert!(
        report
            .routes
            .iter()
            .any(|route| route.family == "CommonModule")
    );
}
