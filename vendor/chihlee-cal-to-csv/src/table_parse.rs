use std::collections::HashMap;

pub(crate) fn split_line_into_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut whitespace_run = 0_usize;

    for ch in trimmed.chars() {
        if ch == '\t' {
            if !current.trim().is_empty() {
                cells.push(current.trim().to_string());
                current.clear();
            }
            whitespace_run = 0;
            continue;
        }

        if ch.is_whitespace() {
            whitespace_run += 1;
            if whitespace_run >= 2 {
                if !current.trim().is_empty() {
                    cells.push(current.trim().to_string());
                    current.clear();
                }
                continue;
            }
            current.push(' ');
            continue;
        }

        whitespace_run = 0;
        current.push(ch);
    }

    if !current.trim().is_empty() {
        cells.push(current.trim().to_string());
    }

    cells
}

pub(crate) fn soft_split_line_into_cells(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

pub(crate) fn normalize_rows(rows: &[Vec<String>], width: usize) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| {
            let mut out = row.clone();
            out.resize(width, String::new());
            out
        })
        .collect()
}

pub(crate) fn modal_width(rows: &[Vec<String>]) -> usize {
    let mut freq = HashMap::new();
    for width in rows.iter().map(Vec::len) {
        *freq.entry(width).or_insert(0_usize) += 1;
    }

    freq.into_iter()
        .max_by_key(|(width, count)| (*count, *width))
        .map_or(0, |(width, _)| width)
}

#[cfg(test)]
mod tests {
    use super::{modal_width, normalize_rows, soft_split_line_into_cells, split_line_into_cells};

    #[test]
    fn splits_double_space_separated_cells() {
        let cells = split_line_into_cells("Alice  30  98");
        assert_eq!(cells, vec!["Alice", "30", "98"]);
    }

    #[test]
    fn splits_tab_separated_cells() {
        let cells = split_line_into_cells("A\tB\tC");
        assert_eq!(cells, vec!["A", "B", "C"]);
    }

    #[test]
    fn soft_splits_single_space_cells() {
        let cells = soft_split_line_into_cells("Name Age Score");
        assert_eq!(cells, vec!["Name", "Age", "Score"]);
    }

    #[test]
    fn normalizes_ragged_rows() {
        let rows = vec![
            vec!["a".to_string()],
            vec!["b".to_string(), "c".to_string()],
        ];
        let normalized = normalize_rows(&rows, 3);
        assert_eq!(normalized[0], vec!["a", "", ""]);
        assert_eq!(normalized[1], vec!["b", "c", ""]);
    }

    #[test]
    fn detects_modal_width() {
        let rows = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["1".to_string(), "2".to_string()],
            vec!["x".to_string()],
        ];
        assert_eq!(modal_width(&rows), 2);
    }
}
