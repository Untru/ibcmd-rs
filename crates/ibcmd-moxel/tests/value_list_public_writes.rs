use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ibcmd_moxel::{
    MoxelCell, MoxelCellValue, MoxelRow, parse_moxel_cell, parse_moxel_cell_value,
    push_moxel_row_xml,
};

fn malformed_value() -> String {
    ibcmd_schema::spreadsheet_value_list::empty_stored_value()
        .replace("\"Pattern\"", "\"UnmeasuredPattern\"")
}

fn row(cells: Vec<MoxelCell>) -> MoxelRow {
    MoxelRow {
        index: 0,
        index_to: None,
        format_index: 0,
        source_format_index: None,
        columns_id: None,
        cells,
    }
}

fn unmeasured_cells() -> Vec<MoxelCell> {
    let raw = malformed_value();
    let value = parse_moxel_cell_value(&raw).unwrap();
    let mut cells = vec![
        parse_moxel_cell(&format!("{{2,0,{raw}}}"), 1).unwrap(),
        parse_moxel_cell(&format!("{{4,0,{raw}}}"), 1).unwrap(),
    ];
    for nested in [
        MoxelCellValue::Array(vec![MoxelCellValue::Text("before".into()), value.clone()]),
        MoxelCellValue::Structure(vec![
            ("before".into(), MoxelCellValue::Text("kept".into())),
            ("unknown".into(), MoxelCellValue::Array(vec![value])),
        ]),
    ] {
        for detail in [false, true] {
            let mut cell = parse_moxel_cell("{2,0,{\"S\",\"known\"}}", 1).unwrap();
            if detail {
                cell.detail_value = Some(nested.clone());
            } else {
                cell.value = Some(nested.clone());
            }
            cells.push(cell);
        }
    }
    cells
}

#[test]
fn public_row_write_refuses_unmeasured_values_without_panicking_or_modifying_the_buffer() {
    for cell in unmeasured_cells() {
        let row = row(vec![
            parse_moxel_cell("{2,0,{\"S\",\"first cell\"}}", 0).unwrap(),
            cell,
        ]);
        let mut buffer = String::from("previous XML stays exact");
        let result = catch_unwind(AssertUnwindSafe(|| {
            push_moxel_row_xml(&mut buffer, &row, &BTreeMap::new(), false)
        }));
        let diagnostic = result
            .expect("the public writer must not panic")
            .expect_err("an unmeasured value must return an explicit refusal");
        assert_eq!(diagnostic.stage(), ibcmd_moxel::MxlDiagnosticStage::Writer);
        assert_eq!(diagnostic.code(), "mxl.writer.value-list-profile");
        assert_eq!(
            buffer, "previous XML stays exact",
            "the public writer must refuse the entire row before appending XML"
        );
    }
}

#[test]
fn public_row_write_appends_the_complete_supported_row() {
    let row = row(vec![
        parse_moxel_cell("{2,0,{\"S\",\"known\"}}", 0).unwrap(),
    ]);
    let mut buffer = String::from("previous XML");
    push_moxel_row_xml(&mut buffer, &row, &BTreeMap::new(), false).unwrap();
    assert_eq!(
        buffer,
        "previous XML\t<rowsItem>\r\n\t\t<index>0</index>\r\n\t\t<row>\r\n\t\t\t<c>\r\n\t\t\t\t<c>\r\n\t\t\t\t\t<f>0</f>\r\n\t\t\t\t\t<v xsi:type=\"xs:string\">known</v>\r\n\t\t\t\t</c>\r\n\t\t\t</c>\r\n\t\t</row>\r\n\t</rowsItem>\r\n"
    );
}
