use crate::model::DetectedTable;
use crate::options::HeaderMode;
use crate::warning::{ExtractWarning, WarningCode};

fn is_numeric(value: &str) -> bool {
    let trimmed = value.trim().replace(',', "");
    trimmed.parse::<f64>().is_ok()
}

fn non_numeric_ratio(cells: &[String]) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }

    let non_numeric = cells.iter().filter(|cell| !is_numeric(cell)).count();
    non_numeric as f32 / cells.len() as f32
}

pub(crate) fn infer_has_header(rows: &[Vec<String>]) -> (bool, f32) {
    if rows.is_empty() {
        return (false, 0.0);
    }

    let first = non_numeric_ratio(&rows[0]);
    let second = rows.get(1).map_or(0.0, |row| non_numeric_ratio(row));

    let confidence = (first * 0.6 + (1.0 - second) * 0.4).clamp(0.0, 1.0);
    let has_header = first >= 0.6 && second <= 0.7;
    (has_header, confidence)
}

pub(crate) fn apply_header_mode(
    table: &DetectedTable,
    mode: HeaderMode,
    warnings: &mut Vec<ExtractWarning>,
    table_id: usize,
) -> Vec<Vec<String>> {
    if table.rows.is_empty() {
        return Vec::new();
    }

    match mode {
        HeaderMode::HasHeader => table.rows.iter().skip(1).cloned().collect(),
        HeaderMode::NoHeader => table.rows.clone(),
        HeaderMode::AutoDetect => {
            let (has_header, confidence) = infer_has_header(&table.rows);
            if has_header && confidence >= 0.55 {
                return table.rows.iter().skip(1).cloned().collect();
            }

            if confidence < 0.55 {
                warnings.push(
                    ExtractWarning::new(
                        WarningCode::HeaderInferenceLowConfidence,
                        "header inference confidence is low; keeping the first row as data",
                    )
                    .with_page(table.page)
                    .with_table_id(table_id)
                    .with_confidence(confidence),
                );
            }

            table.rows.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::header::infer_has_header;

    #[test]
    fn infers_headers_for_text_then_numeric_rows() {
        let rows = vec![
            vec!["Name".to_string(), "Age".to_string()],
            vec!["Alice".to_string(), "30".to_string()],
        ];
        let (has_header, confidence) = infer_has_header(&rows);
        assert!(has_header);
        assert!(confidence > 0.5);
    }
}
