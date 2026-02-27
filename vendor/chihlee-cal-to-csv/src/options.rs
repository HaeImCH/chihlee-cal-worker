use std::collections::BTreeSet;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderMode {
    AutoDetect,
    HasHeader,
    NoHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityMode {
    BestEffort,
    Strict,
    SkipAmbiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSelection {
    pages: BTreeSet<u32>,
}

impl PageSelection {
    #[must_use]
    pub fn contains(&self, page: u32) -> bool {
        self.pages.contains(&page)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

impl FromStr for PageSelection {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        let mut pages = BTreeSet::new();
        for token in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if let Some((start, end)) = token.split_once('-') {
                let start: u32 = start
                    .trim()
                    .parse()
                    .map_err(|_| format!("invalid page range start: '{start}'"))?;
                let end: u32 = end
                    .trim()
                    .parse()
                    .map_err(|_| format!("invalid page range end: '{end}'"))?;
                if start == 0 || end == 0 {
                    return Err("pages are 1-based".to_string());
                }
                if end < start {
                    return Err(format!(
                        "invalid range '{token}': end is smaller than start"
                    ));
                }
                pages.extend(start..=end);
            } else {
                let page: u32 = token
                    .parse()
                    .map_err(|_| format!("invalid page number: '{token}'"))?;
                if page == 0 {
                    return Err("pages are 1-based".to_string());
                }
                pages.insert(page);
            }
        }

        if pages.is_empty() {
            return Err("page selection cannot be empty".to_string());
        }

        Ok(Self { pages })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableArea {
    pub page: u32,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl FromStr for TableArea {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        let (page_part, rect_part) = spec
            .split_once(':')
            .ok_or_else(|| format!("invalid area format '{spec}', expected page:x1,y1,x2,y2"))?;

        let page: u32 = page_part
            .trim()
            .parse()
            .map_err(|_| format!("invalid page number in area: '{page_part}'"))?;

        if page == 0 {
            return Err("area page number must be >= 1".to_string());
        }

        let parts = rect_part.split(',').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "invalid area format '{spec}', expected exactly 4 coordinates"
            ));
        }

        let x1: f32 = parts[0]
            .parse()
            .map_err(|_| format!("invalid x1 coordinate: '{}'", parts[0]))?;
        let y1: f32 = parts[1]
            .parse()
            .map_err(|_| format!("invalid y1 coordinate: '{}'", parts[1]))?;
        let x2: f32 = parts[2]
            .parse()
            .map_err(|_| format!("invalid x2 coordinate: '{}'", parts[2]))?;
        let y2: f32 = parts[3]
            .parse()
            .map_err(|_| format!("invalid y2 coordinate: '{}'", parts[3]))?;

        if x2 <= x1 || y2 <= y1 {
            return Err("area requires x2>x1 and y2>y1".to_string());
        }

        Ok(Self {
            page,
            x1,
            y1,
            x2,
            y2,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractOptions {
    pub pages: Option<PageSelection>,
    pub areas: Vec<TableArea>,
    pub delimiter: u8,
    pub header_mode: HeaderMode,
    pub quality_mode: QualityMode,
    pub min_cols: usize,
    pub clean_calendar: bool,
    pub no_page: bool,
    pub no_table: bool,
    pub custom_col_names: Option<(String, String)>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            pages: None,
            areas: Vec::new(),
            delimiter: b',',
            header_mode: HeaderMode::AutoDetect,
            quality_mode: QualityMode::BestEffort,
            min_cols: 2,
            clean_calendar: false,
            no_page: false,
            no_table: false,
            custom_col_names: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PageSelection, TableArea};
    use std::str::FromStr;

    #[test]
    fn parse_page_selection_range_and_single() {
        let selection = PageSelection::from_str("1-3,5").expect("selection should parse");
        assert!(selection.contains(1));
        assert!(selection.contains(2));
        assert!(selection.contains(3));
        assert!(selection.contains(5));
        assert!(!selection.contains(4));
    }

    #[test]
    fn reject_invalid_page_selection() {
        let err = PageSelection::from_str("3-1").expect_err("invalid range should fail");
        assert!(err.contains("invalid range"));
    }

    #[test]
    fn parse_table_area() {
        let area = TableArea::from_str("2:10,20,120,220").expect("area should parse");
        assert_eq!(area.page, 2);
        assert_eq!(area.x1, 10.0);
        assert_eq!(area.y2, 220.0);
    }

    #[test]
    fn reject_invalid_table_area() {
        let err = TableArea::from_str("1:0,0,10").expect_err("invalid area should fail");
        assert!(err.contains("expected exactly 4 coordinates"));
    }
}
