#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningCode {
    LowConfidence,
    HeaderInferenceLowConfidence,
    AreaFallbackApproximate,
    NoTablesDetected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractWarning {
    pub code: WarningCode,
    pub message: String,
    pub page: Option<u32>,
    pub table_id: Option<usize>,
    pub confidence: Option<f32>,
}

impl ExtractWarning {
    #[must_use]
    pub fn new(code: WarningCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            page: None,
            table_id: None,
            confidence: None,
        }
    }

    #[must_use]
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    #[must_use]
    pub fn with_table_id(mut self, table_id: usize) -> Self {
        self.table_id = Some(table_id);
        self
    }

    #[must_use]
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }
}
