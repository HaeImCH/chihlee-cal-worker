mod common;

use std::process::Command;

use chihlee_cal_to_csv::{ExtractOptions, TableArea, extract_pdf_to_csv};
use tempfile::tempdir;

#[test]
fn extracts_single_table_to_merged_csv() {
    let dir = tempdir().expect("tempdir should be created");
    let input = dir.path().join("single.pdf");
    let output = dir.path().join("single.csv");

    common::create_test_pdf(
        &input,
        &[vec!["Name  Age  Score", "Alice  30  98", "Bob  22  87"]],
    )
    .expect("PDF fixture should be created");

    let report = extract_pdf_to_csv(&input, &output, &ExtractOptions::default())
        .expect("extraction should succeed");

    let csv = std::fs::read_to_string(&output).expect("CSV should be readable");
    assert!(
        csv.contains("page,table_id,col_1,col_2,col_3"),
        "unexpected CSV output: {csv:?}, report: {report:?}"
    );
    assert!(
        csv.contains("Alice,30,98"),
        "unexpected CSV output: {csv:?}, report: {report:?}"
    );
    assert_eq!(report.table_count, 1);
    assert_eq!(report.row_count, 2);
}

#[test]
fn merges_tables_from_multiple_pages() {
    let dir = tempdir().expect("tempdir should be created");
    let input = dir.path().join("multi.pdf");
    let output = dir.path().join("multi.csv");

    common::create_test_pdf(
        &input,
        &[
            vec!["City  Pop  Rank", "A  10  1", "B  20  2"],
            vec!["Product  Qty  Price", "Pen  3  1.5", "Book  1  9.9"],
        ],
    )
    .expect("PDF fixture should be created");

    let report = extract_pdf_to_csv(&input, &output, &ExtractOptions::default())
        .expect("extraction should succeed");

    let csv = std::fs::read_to_string(&output).expect("CSV should be readable");
    assert!(
        csv.contains("1,1"),
        "unexpected CSV output: {csv:?}, report: {report:?}"
    );
    assert!(
        csv.contains("2,2"),
        "unexpected CSV output: {csv:?}, report: {report:?}"
    );
    assert_eq!(report.table_count, 2);
    assert_eq!(report.row_count, 4);
}

#[test]
fn warns_on_ambiguous_table_structure() {
    let dir = tempdir().expect("tempdir should be created");
    let input = dir.path().join("ambiguous.pdf");
    let output = dir.path().join("ambiguous.csv");

    common::create_test_pdf(&input, &[vec!["A  B  C", "1  2", "3  4  5  6", "7  8"]])
        .expect("PDF fixture should be created");

    let report = extract_pdf_to_csv(&input, &output, &ExtractOptions::default())
        .expect("extraction should succeed");

    assert!(!report.warnings.is_empty());
}

#[test]
fn manual_area_can_recover_detection_with_strict_min_cols() {
    let dir = tempdir().expect("tempdir should be created");
    let input = dir.path().join("area.pdf");
    let output = dir.path().join("area.csv");

    common::create_test_pdf(
        &input,
        &[vec!["Name  Age  Score", "Alice  30  98", "Bob  22  87"]],
    )
    .expect("PDF fixture should be created");

    let mut options = ExtractOptions {
        min_cols: 4,
        ..ExtractOptions::default()
    };
    options.areas.push(
        "1:10,20,100,200"
            .parse::<TableArea>()
            .expect("area should parse"),
    );

    let report = extract_pdf_to_csv(&input, &output, &options).expect("extraction should succeed");
    assert!(report.row_count > 0, "report: {report:?}");
}

#[test]
fn returns_no_rows_for_non_table_pdf() {
    let dir = tempdir().expect("tempdir should be created");
    let input = dir.path().join("notable.pdf");
    let output = dir.path().join("notable.csv");

    common::create_test_pdf(
        &input,
        &[vec!["This is plain narrative text without columns."]],
    )
    .expect("PDF fixture should be created");

    let report = extract_pdf_to_csv(&input, &output, &ExtractOptions::default())
        .expect("extraction should succeed");
    assert_eq!(report.row_count, 0);
    assert_eq!(report.table_count, 0);
}

#[test]
fn cli_exits_with_code_2_when_no_rows() {
    let dir = tempdir().expect("tempdir should be created");
    let input = dir.path().join("cli-empty.pdf");
    let output = dir.path().join("cli-empty.csv");

    common::create_test_pdf(&input, &[vec!["No table here"]])
        .expect("PDF fixture should be created");

    let status = Command::new(env!("CARGO_BIN_EXE_pdf2csv"))
        .args([
            "extract",
            "-i",
            &input.to_string_lossy(),
            "-o",
            &output.to_string_lossy(),
        ])
        .status()
        .expect("CLI should run");

    assert_eq!(status.code(), Some(2));
}
