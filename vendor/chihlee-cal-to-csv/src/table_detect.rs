use std::collections::BTreeSet;

use crate::model::{DetectedTable, PageText, TableOrigin};
use crate::options::ExtractOptions;
use crate::table_parse::{modal_width, soft_split_line_into_cells, split_line_into_cells};
use crate::warning::{ExtractWarning, WarningCode};

pub(crate) const LOW_CONFIDENCE_THRESHOLD: f32 = 0.60;

fn table_confidence(rows: &[Vec<String>]) -> f32 {
    if rows.len() < 2 {
        return 0.0;
    }

    let modal = modal_width(rows);
    if modal == 0 {
        return 0.0;
    }

    let consistent =
        rows.iter().filter(|row| row.len() == modal).count() as f32 / rows.len() as f32;
    let max_width = rows.iter().map(Vec::len).max().unwrap_or(modal);
    let min_width = rows.iter().map(Vec::len).min().unwrap_or(modal);
    let uniformity = if max_width == 0 {
        0.0
    } else {
        1.0 - ((max_width - min_width) as f32 / max_width as f32)
    };

    (consistent * 0.75 + uniformity * 0.25).clamp(0.0, 1.0)
}

fn detect_tables_in_page(
    page: &PageText,
    min_cols: usize,
    origin: TableOrigin,
) -> Vec<DetectedTable> {
    let mut tables = Vec::new();
    let mut current_rows: Vec<Vec<String>> = Vec::new();

    let flush_current = |rows: &mut Vec<Vec<String>>, tables: &mut Vec<DetectedTable>| {
        if rows.len() >= 2 {
            let confidence = table_confidence(rows);
            tables.push(DetectedTable {
                page: page.page_number,
                rows: std::mem::take(rows),
                confidence,
                origin,
            });
        } else {
            rows.clear();
        }
    };

    for line in page.text.lines() {
        let mut cells = split_line_into_cells(line);
        if cells.len() < min_cols {
            let soft_cells = soft_split_line_into_cells(line);
            let has_numeric = soft_cells
                .iter()
                .any(|cell| cell.chars().any(|ch| ch.is_ascii_digit()));
            let looks_like_sentence = ['.', '!', '?']
                .iter()
                .any(|punctuation| line.trim_end().ends_with(*punctuation));
            if soft_cells.len() >= min_cols
                && !looks_like_sentence
                && (has_numeric || soft_cells.len() <= 6)
            {
                cells = soft_cells;
            }
        }

        if cells.len() >= min_cols {
            current_rows.push(cells);
        } else {
            flush_current(&mut current_rows, &mut tables);
        }
    }

    flush_current(&mut current_rows, &mut tables);
    tables
}

fn detect_using_manual_areas(
    pages: &[PageText],
    options: &ExtractOptions,
    warnings: &mut Vec<ExtractWarning>,
) -> Vec<DetectedTable> {
    let relaxed_min_cols = options.min_cols.saturating_sub(1).max(2);
    let area_pages: BTreeSet<u32> = options.areas.iter().map(|area| area.page).collect();

    let mut manual_tables = Vec::new();
    for page_no in area_pages {
        if let Some(page) = pages
            .iter()
            .find(|candidate| candidate.page_number == page_no)
        {
            manual_tables.extend(detect_tables_in_page(
                page,
                relaxed_min_cols,
                TableOrigin::ManualArea,
            ));
        } else {
            warnings.push(
                ExtractWarning::new(
                    WarningCode::AreaFallbackApproximate,
                    "manual area page is not present in selected PDF pages",
                )
                .with_page(page_no),
            );
        }
    }

    manual_tables
}

pub(crate) fn detect_tables(
    pages: &[PageText],
    options: &ExtractOptions,
    warnings: &mut Vec<ExtractWarning>,
) -> Vec<DetectedTable> {
    let mut auto_tables = Vec::new();
    for page in pages {
        auto_tables.extend(detect_tables_in_page(
            page,
            options.min_cols.max(2),
            TableOrigin::Auto,
        ));
    }

    let has_low_confidence = auto_tables
        .iter()
        .any(|table| table.confidence < LOW_CONFIDENCE_THRESHOLD);

    if options.areas.is_empty() {
        return auto_tables;
    }

    if auto_tables.is_empty() || has_low_confidence {
        warnings.push(ExtractWarning::new(
            WarningCode::AreaFallbackApproximate,
            "manual area fallback uses page-level extraction because pdf-extract does not expose table geometry",
        ));
    }

    if auto_tables.is_empty() {
        return detect_using_manual_areas(pages, options, warnings);
    }

    if has_low_confidence {
        let mut filtered = auto_tables
            .into_iter()
            .filter(|table| table.confidence >= LOW_CONFIDENCE_THRESHOLD)
            .collect::<Vec<_>>();
        filtered.extend(detect_using_manual_areas(pages, options, warnings));
        return filtered;
    }

    auto_tables
}
