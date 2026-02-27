use crate::model::{MergedOutput, PreparedTable};
use crate::table_parse::normalize_rows;

pub(crate) fn merge_tables(tables: &[PreparedTable]) -> MergedOutput {
    let width = tables
        .iter()
        .flat_map(|table| table.rows.iter().map(Vec::len))
        .max()
        .unwrap_or(0);

    let mut headers = vec!["page".to_string(), "table_id".to_string()];
    headers.extend((1..=width).map(|index| format!("col_{index}")));

    let mut rows = Vec::new();
    for table in tables {
        let normalized = normalize_rows(&table.rows, width);
        for data_row in normalized {
            let mut row = Vec::with_capacity(width + 2);
            row.push(table.page.to_string());
            row.push(table.table_id.to_string());
            row.extend(data_row);
            rows.push(row);
        }
    }

    MergedOutput {
        headers,
        row_count: rows.len(),
        table_count: tables.len(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use crate::merge::merge_tables;
    use crate::model::PreparedTable;

    #[test]
    fn merges_and_pads_rows_to_global_schema() {
        let tables = vec![PreparedTable {
            page: 1,
            table_id: 1,
            rows: vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string()],
            ],
        }];

        let merged = merge_tables(&tables);
        assert_eq!(merged.headers, vec!["page", "table_id", "col_1", "col_2"]);
        assert_eq!(merged.rows[1], vec!["1", "1", "c", ""]);
    }
}
