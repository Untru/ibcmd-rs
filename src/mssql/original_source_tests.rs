//! These exercise the same private encoder/read handlers used by source apply.
use super::*;
use crate::mssql_source_change::HeldSourceRoot;
use std::sync::Arc;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("ibcmd-compiler-original-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
    fn source(&self) -> MetadataSourceContext {
        MetadataSourceContext::with_original_source(Arc::new(
            HeldSourceRoot::open_compiler_operation(&self.0).unwrap(),
        ))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("ibcmd-compiler-original-")
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn assert_failed_original_source_cannot_write_script(source: &MetadataSourceContext) {
    let output = Fixture::new();
    let kept = output.write("kept.sql", b"accepted script prefix");
    let refused = output.0.join("never-created/refused.sql");
    let build_called = std::cell::Cell::new(false);
    assert!(
        write_original_source_script(source, &refused, || {
            build_called.set(true);
            "SELECT 1;".to_owned()
        })
        .is_err()
    );
    assert!(!build_called.get());
    assert!(!refused.parent().unwrap().exists());
    assert!(!refused.exists());
    assert_eq!(fs::read(kept).unwrap(), b"accepted script prefix");
}

#[test]
fn actual_module_encoder_consumes_owned_original_in_both_dialects() {
    let fixture = Fixture::new();
    let path = fixture.write(
        "CommonModules/M/Ext/Module.bsl",
        b"Procedure Example() Export\r\nEndProcedure\r\n",
    );
    let source = fixture.source();
    for dialect in ["2.20", "2.21"] {
        let axes = mssql_compile_axes(XmlDialect::parse(dialect).unwrap());
        let key = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0";
        // Positive actual encoder before integrity-negative attempts.
        let ordinary = pack_module_body_source(&path, key, &axes).unwrap();
        let owned = pack_module_body_source_with_source(&path, key, &axes, Some(&source)).unwrap();
        assert_eq!(owned.blob, ordinary.blob);
        assert_eq!(owned.output_sha256, ordinary.output_sha256);
        assert_eq!(owned.text_bytes, ordinary.text_bytes);
    }
    source.require_original_unchanged().unwrap();
    for dialect in ["2.20", "2.21"] {
        let axes = mssql_compile_axes(XmlDialect::parse(dialect).unwrap());
        let key = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0";
        assert!(
            pack_module_body_source_with_source(
                &fixture.0.join("late.bsl"),
                key,
                &axes,
                Some(&source)
            )
            .is_err()
        );
    }
    assert!(source.require_original_reads().is_err());
    assert!(source.require_original_unchanged().is_err());
    assert_failed_original_source_cannot_write_script(&source);
}

#[test]
fn actual_html_handler_reads_pages_and_attachments_through_same_owner() {
    let fixture = Fixture::new();
    let body = fixture.write("Reports/R/Templates/Page/Ext/Template.xml",
        br#"<?xml version="1.0" encoding="UTF-8"?><Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"><Page>index</Page></Help>"#);
    fixture.write(
        "Reports/R/Templates/Page/Ext/Template/index.html",
        b"<html><body>Example</body></html>",
    );
    fixture.write(
        "Reports/R/Templates/Page/Ext/Template/_files/a.txt",
        b"attachment",
    );
    let source = fixture.source();
    let ordinary =
        read_help_source_parts(&body, "Template", HtmlPageOwner::Template, None).unwrap();
    let owned =
        read_help_source_parts(&body, "Template", HtmlPageOwner::Template, Some(&source)).unwrap();
    assert_eq!(owned, ordinary);
    assert_eq!(owned.0.len(), 1);
    assert_eq!(owned.1, vec![("a.txt".to_owned(), b"attachment".to_vec())]);
    fixture.write(
        "Reports/R/Templates/Page/Ext/Template/_files/late.txt",
        b"not in census",
    );
    let repeated =
        read_help_source_parts(&body, "Template", HtmlPageOwner::Template, Some(&source)).unwrap();
    assert_eq!(repeated, owned);
    assert!(source.require_original_unchanged().is_err());
    assert_failed_original_source_cannot_write_script(&source);
}

#[test]
fn actual_module_encoder_does_not_use_new_unadmitted_file_or_another_context() {
    let first = Fixture::new();
    let second = Fixture::new();
    let relative = "CommonModules/M/Ext/Module.bsl";
    let first_path = first.write(relative, b"// FIRST\r\n");
    let second_path = second.write(relative, b"// SECOND\r\n");
    let source = first.source();
    let axes = legacy_non_xml_compile_axes();
    let key = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0";
    assert!(pack_module_body_source_with_source(&first_path, key, &axes, Some(&source)).is_ok());
    assert!(pack_module_body_source_with_source(&second_path, key, &axes, Some(&source)).is_err());
    let late = first.write(
        "late.bsl",
        b"// actual file exists but was not admitted\r\n",
    );
    assert!(pack_module_body_source_with_source(&late, key, &axes, Some(&source)).is_err());
}
