use morph1c_core::{
    ir::{MetadataObject, ObjectKind, PictureBody, PropertyValue, Uuid},
    spec::metadata::common_picture::F_TRANSPARENT_PIXEL,
    version::FormatVersion,
};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};

fn config(pixel: Option<(i64, i64)>) -> morph1c_core::ir::Configuration {
    let options = ConvertOptions::default().with_target_version(FormatVersion::new(2, 21));
    let mut cfg = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &options,
    )
    .unwrap()
    .0;
    let mut pic = MetadataObject::new(
        ObjectKind::new("CommonPicture"),
        "SentinelPicture",
        Uuid([67; 16]),
    );
    if let Some((x, y)) = pixel {
        pic.properties.push((
            F_TRANSPARENT_PIXEL,
            PropertyValue::List(vec![PropertyValue::Int(x), PropertyValue::Int(y)]),
        ));
    }
    pic.picture = Some(PictureBody {
        file_name: "Picture.png".into(),
        bytes: vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15,
            0, 1, 5, 1, 1, 39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ],
    });
    cfg.objects.push(pic);
    cfg
}
fn picture(cfg: &morph1c_core::ir::Configuration) -> &MetadataObject {
    cfg.objects
        .iter()
        .find(|o| o.name == "SentinelPicture")
        .unwrap()
}
fn read(format: Format, dir: &std::path::Path) -> morph1c_core::ir::Configuration {
    read_config(
        format,
        dir,
        &ConvertOptions::default().with_target_version(FormatVersion::new(2, 21)),
    )
    .unwrap()
    .0
}
#[test]
fn official_empty_transparent_point_roundtrips_both_formats_without_inventing_pixel() {
    let original = config(Some((-1, -1)));
    let native = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &original, native.path()).unwrap();
    let path = native
        .path()
        .join("CommonPictures/SentinelPicture/Ext/Picture.xml");
    let bytes = std::fs::read(&path).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains("<xr:LoadTransparent>true</xr:LoadTransparent>"));
    assert!(!text.contains("<xr:TransparentPixel"));
    let native_model = read(Format::Designer, native.path());
    assert_eq!(
        picture(&native_model).get(F_TRANSPARENT_PIXEL),
        picture(&original).get(F_TRANSPARENT_PIXEL)
    );
    assert_eq!(picture(&native_model).picture, picture(&original).picture);
    let edt = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &native_model, edt.path()).unwrap();
    let edt_model = read(Format::Edt, edt.path());
    assert_eq!(
        picture(&edt_model).get(F_TRANSPARENT_PIXEL),
        picture(&original).get(F_TRANSPARENT_PIXEL)
    );
    assert_eq!(picture(&edt_model).picture, picture(&original).picture);
    let returned = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &edt_model, returned.path()).unwrap();
    assert_eq!(
        std::fs::read(
            returned
                .path()
                .join("CommonPictures/SentinelPicture/Ext/Picture.xml")
        )
        .unwrap(),
        bytes
    );
}
#[test]
fn current_transparent_point_edit_changes_complete_native_output() {
    let mut current = config(Some((-1, -1)));
    let native = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &current, native.path()).unwrap();
    current = read(Format::Designer, native.path());
    let pic = current
        .objects
        .iter_mut()
        .find(|o| o.name == "SentinelPicture")
        .unwrap();
    pic.properties
        .iter_mut()
        .find(|(id, _)| *id == F_TRANSPARENT_PIXEL)
        .unwrap()
        .1 = PropertyValue::List(vec![PropertyValue::Int(13), PropertyValue::Int(3)]);
    let edited = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &current, edited.path()).unwrap();
    let text = std::fs::read_to_string(
        edited
            .path()
            .join("CommonPictures/SentinelPicture/Ext/Picture.xml"),
    )
    .unwrap();
    assert!(text.contains("<xr:TransparentPixel x=\"13\" y=\"3\"/>"));
    assert_eq!(
        picture(&read(Format::Designer, edited.path())).get(F_TRANSPARENT_PIXEL),
        picture(&current).get(F_TRANSPARENT_PIXEL)
    );
}
#[test]
fn native_false_with_explicit_pixel_is_not_silently_discarded() {
    let native = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &config(Some((13, 3))), native.path()).unwrap();
    let path = native
        .path()
        .join("CommonPictures/SentinelPicture/Ext/Picture.xml");
    let text = std::fs::read_to_string(&path).unwrap().replace(
        "<xr:LoadTransparent>true</xr:LoadTransparent>",
        "<xr:LoadTransparent>false</xr:LoadTransparent>",
    );
    std::fs::write(&path, text).unwrap();
    assert!(read_config(Format::Designer, native.path(), &ConvertOptions::default()).is_err());
}

#[test]
fn explicit_native_sentinel_spelling_is_exact_and_current_edits_invalidate_presence() {
    let native = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &config(Some((-1, -1))), native.path()).unwrap();
    let relative = "CommonPictures/SentinelPicture/Ext/Picture.xml";
    let path = native.path().join(relative);
    let original = std::fs::read_to_string(&path).unwrap().replace(
        "</xr:LoadTransparent>",
        "</xr:LoadTransparent>\r\n\t\t<xr:TransparentPixel x=\"-1\" y=\"-1\"/>",
    );
    std::fs::write(&path, &original).unwrap();
    let model = read(Format::Designer, native.path());
    let unchanged = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &model, unchanged.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(unchanged.path().join(relative)).unwrap(),
        original
    );
    for replacement in [Some((13, 3)), None] {
        let mut edited = model.clone();
        let pic = edited
            .objects
            .iter_mut()
            .find(|o| o.name == "SentinelPicture")
            .unwrap();
        pic.properties.retain(|(id, _)| *id != F_TRANSPARENT_PIXEL);
        if let Some((x, y)) = replacement {
            pic.properties.push((
                F_TRANSPARENT_PIXEL,
                PropertyValue::List(vec![PropertyValue::Int(x), PropertyValue::Int(y)]),
            ));
        }
        let out = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &edited, out.path()).unwrap();
        let text = std::fs::read_to_string(out.path().join(relative)).unwrap();
        assert!(!text.contains("x=\"-1\" y=\"-1\""));
        if replacement.is_some() {
            assert!(text.contains("<xr:TransparentPixel x=\"13\" y=\"3\"/>"));
        } else {
            assert!(text.contains("<xr:LoadTransparent>false</xr:LoadTransparent>"));
            assert!(!text.contains("<xr:TransparentPixel"));
        }
        assert_eq!(
            picture(&read(Format::Designer, out.path())).get(F_TRANSPARENT_PIXEL),
            picture(&edited).get(F_TRANSPARENT_PIXEL)
        );
    }
}
