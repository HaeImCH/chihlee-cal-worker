#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageText {
    pub page_number: u32,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableOrigin {
    Auto,
    ManualArea,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedTable {
    pub page: u32,
    pub rows: Vec<Vec<String>>,
    pub confidence: f32,
    pub origin: TableOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTable {
    pub page: u32,
    pub table_id: usize,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedOutput {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub table_count: usize,
    pub row_count: usize,
}
