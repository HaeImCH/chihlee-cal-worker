mod clean_calendar;
mod csv_out;
mod error;
mod header;
mod merge;
mod model;
mod options;
mod pdf_reader;
mod table_detect;
mod table_parse;
mod warning;

use std::path::Path;

use crate::csv_out::{write_csv, write_csv_to_string};
use crate::header::apply_header_mode;
use crate::merge::merge_tables;
use crate::model::{PageText, PreparedTable};
use crate::pdf_reader::{read_pdf_pages, read_pdf_pages_from_bytes};
use crate::table_detect::{LOW_CONFIDENCE_THRESHOLD, detect_tables};
use crate::warning::WarningCode;

pub use error::ExtractError;
pub use options::{ExtractOptions, HeaderMode, PageSelection, QualityMode, TableArea};
pub use warning::{ExtractWarning, WarningCode as ExtractWarningCode};

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractionReport {
    pub row_count: usize,
    pub table_count: usize,
    pub warnings: Vec<ExtractWarning>,
}

fn apply_output_column_filters(
    merged: crate::model::MergedOutput,
    options: &ExtractOptions,
) -> crate::model::MergedOutput {
    if !options.no_page && !options.no_table {
        return merged;
    }

    let keep_indices = merged
        .headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| {
            if options.no_page && header == "page" {
                return None;
            }
            if options.no_table && header == "table_id" {
                return None;
            }
            Some(index)
        })
        .collect::<Vec<_>>();

    let headers = keep_indices
        .iter()
        .map(|&index| merged.headers[index].clone())
        .collect::<Vec<_>>();
    let rows = merged
        .rows
        .iter()
        .map(|row| {
            keep_indices
                .iter()
                .filter_map(|&index| row.get(index).cloned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    crate::model::MergedOutput {
        headers,
        rows,
        row_count: merged.row_count,
        table_count: merged.table_count,
    }
}

fn apply_custom_column_names(
    mut merged: crate::model::MergedOutput,
    options: &ExtractOptions,
) -> crate::model::MergedOutput {
    let Some((col1_name, col2_name)) = &options.custom_col_names else {
        return merged;
    };

    for header in &mut merged.headers {
        if header == "col_1" {
            *header = col1_name.clone();
        } else if header == "col_2" {
            *header = col2_name.clone();
        }
    }

    merged
}

fn apply_quality_mode(
    tables: Vec<crate::model::DetectedTable>,
    options: &ExtractOptions,
    warnings: &mut Vec<ExtractWarning>,
) -> Result<Vec<crate::model::DetectedTable>, ExtractError> {
    let mut out = Vec::new();

    for table in tables {
        if table.confidence >= LOW_CONFIDENCE_THRESHOLD {
            out.push(table);
            continue;
        }

        match options.quality_mode {
            QualityMode::BestEffort => {
                warnings.push(
                    ExtractWarning::new(
                        WarningCode::LowConfidence,
                        "table confidence is low; exported in best-effort mode",
                    )
                    .with_page(table.page)
                    .with_confidence(table.confidence),
                );
                out.push(table);
            }
            QualityMode::Strict => {
                return Err(ExtractError::AmbiguousTable {
                    page: table.page,
                    confidence: table.confidence,
                });
            }
            QualityMode::SkipAmbiguous => {
                warnings.push(
                    ExtractWarning::new(
                        WarningCode::LowConfidence,
                        "skipping low-confidence table",
                    )
                    .with_page(table.page)
                    .with_confidence(table.confidence),
                );
            }
        }
    }

    Ok(out)
}

fn extract_from_pages(
    pages: &[PageText],
    full_text: Option<&str>,
    options: &ExtractOptions,
) -> Result<(crate::model::MergedOutput, Vec<ExtractWarning>), ExtractError> {
    let mut warnings = Vec::new();
    let mut raw_tables = detect_tables(pages, options, &mut warnings);
    if raw_tables.is_empty()
        && let Some(text) = full_text.filter(|text| !text.trim().is_empty())
    {
        let fallback_pages = vec![PageText {
            page_number: 1,
            text: text.to_string(),
        }];
        let fallback_tables = detect_tables(&fallback_pages, options, &mut warnings);
        if !fallback_tables.is_empty() {
            warnings.push(ExtractWarning::new(
                WarningCode::AreaFallbackApproximate,
                "no page-level tables detected; retried with document-level text extraction",
            ));
            raw_tables = fallback_tables;
        }
    }
    let filtered_tables = apply_quality_mode(raw_tables, options, &mut warnings)?;

    let effective_header_mode =
        if options.clean_calendar && options.header_mode == HeaderMode::AutoDetect {
            HeaderMode::NoHeader
        } else {
            options.header_mode
        };

    let mut prepared_tables = Vec::new();
    for (index, table) in filtered_tables.iter().enumerate() {
        let table_id = index + 1;
        let rows = apply_header_mode(table, effective_header_mode, &mut warnings, table_id);
        if rows.is_empty() {
            continue;
        }

        prepared_tables.push(PreparedTable {
            page: table.page,
            table_id,
            rows,
        });
    }

    if prepared_tables.is_empty() {
        warnings.push(ExtractWarning::new(
            WarningCode::NoTablesDetected,
            "no table rows were detected in the selected pages",
        ));
    }

    let mut merged = merge_tables(&prepared_tables);
    if options.clean_calendar {
        if let Some(text) = full_text {
            let from_text = clean_calendar::clean_calendar_from_text(text);
            merged = if from_text.row_count > 0 {
                from_text
            } else {
                clean_calendar::clean_calendar_output(&merged)
            };
        } else {
            merged = clean_calendar::clean_calendar_output(&merged);
        }
    }
    merged = apply_output_column_filters(merged, options);
    merged = apply_custom_column_names(merged, options);

    Ok((merged, warnings))
}

pub fn extract_pdf_to_csv(
    input_pdf: &Path,
    output_csv: &Path,
    options: &ExtractOptions,
) -> Result<ExtractionReport, ExtractError> {
    if options.min_cols < 2 {
        return Err(ExtractError::InvalidOption(
            "min_cols must be at least 2".to_string(),
        ));
    }

    let pages = read_pdf_pages(input_pdf, options.pages.as_ref())?;
    let full_text = pdf_extract::extract_text(input_pdf).ok();
    let (merged, warnings) = extract_from_pages(&pages, full_text.as_deref(), options)?;
    write_csv(output_csv, &merged, options.delimiter)?;

    Ok(ExtractionReport {
        row_count: merged.row_count,
        table_count: merged.table_count,
        warnings,
    })
}

pub fn extract_pdf_bytes_to_csv_string(
    input_pdf: &[u8],
    options: &ExtractOptions,
) -> Result<(String, ExtractionReport), ExtractError> {
    if options.min_cols < 2 {
        return Err(ExtractError::InvalidOption(
            "min_cols must be at least 2".to_string(),
        ));
    }

    let pages = read_pdf_pages_from_bytes(input_pdf, options.pages.as_ref())?;
    let full_text = pdf_extract::extract_text_from_mem(input_pdf).ok();
    let (merged, warnings) = extract_from_pages(&pages, full_text.as_deref(), options)?;
    let csv = write_csv_to_string(&merged, options.delimiter)?;

    Ok((
        csv,
        ExtractionReport {
            row_count: merged.row_count,
            table_count: merged.table_count,
            warnings,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::{apply_custom_column_names, apply_output_column_filters};
    use crate::ExtractOptions;
    use crate::model::MergedOutput;

    #[test]
    fn drops_page_and_table_columns() {
        let merged = MergedOutput {
            headers: vec![
                "page".to_string(),
                "table_id".to_string(),
                "col_1".to_string(),
            ],
            rows: vec![vec!["1".to_string(), "2".to_string(), "x".to_string()]],
            row_count: 1,
            table_count: 1,
        };

        let options = ExtractOptions {
            no_page: true,
            no_table: true,
            ..ExtractOptions::default()
        };

        let filtered = apply_output_column_filters(merged, &options);
        assert_eq!(filtered.headers, vec!["col_1"]);
        assert_eq!(filtered.rows[0], vec!["x"]);
    }

    #[test]
    fn renames_col1_col2_headers() {
        let merged = MergedOutput {
            headers: vec![
                "page".to_string(),
                "table_id".to_string(),
                "col_1".to_string(),
                "col_2".to_string(),
            ],
            rows: vec![vec![
                "1".to_string(),
                "2".to_string(),
                "2025/1/1".to_string(),
                "event".to_string(),
            ]],
            row_count: 1,
            table_count: 1,
        };

        let options = ExtractOptions {
            custom_col_names: Some(("date".to_string(), "event".to_string())),
            ..ExtractOptions::default()
        };

        let renamed = apply_custom_column_names(merged, &options);
        assert_eq!(renamed.headers, vec!["page", "table_id", "date", "event"]);
    }
}
