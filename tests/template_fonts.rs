//! Standard style fonts 8.5 names and 8.3.27.2214 keeps by code: an 8.3.27
//! dump writes `ref="-59"`, `ref="-51"` (8.5: `style:TitleLevel2`,
//! `style:TitleLevel4`), and a font whose code the writer does not name must
//! still be written -- dropping it shifts every later font index. The fixture
//! is built and dumped by 8.3.27.2214 (`_onecdec/make_template_fonts_fixture.py`).

use crate::common;

use std::fs;

#[test]
fn style_fonts_keep_their_code_in_the_2_20_dialect() {
    let dir = common::fixture("template_fonts");
    let out = common::temp_dir("template-fonts");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!out.join("ConfigDumpInfo.xml").exists());
    let expected = fs::read(dir.join("Template.xml")).unwrap();
    let actual =
        fs::read(out.join("CommonTemplates/ТестРасширение_Макет/Ext/Template.xml")).unwrap();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}
