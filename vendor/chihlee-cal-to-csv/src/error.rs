use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("CSV write error: {0}")]
    Csv(#[from] csv::Error),

    #[error("failed to load PDF: {0}")]
    PdfLoad(#[from] lopdf::Error),

    #[error("failed to extract PDF text: {0}")]
    PdfExtract(String),

    #[error("invalid page selection: {0}")]
    InvalidPageSelection(String),

    #[error("invalid table area: {0}")]
    InvalidTableArea(String),

    #[error("invalid option: {0}")]
    InvalidOption(String),

    #[error("no pages available after applying selection")]
    NoPagesSelected,

    #[error("table on page {page} is too ambiguous (confidence={confidence:.2})")]
    AmbiguousTable { page: u32, confidence: f32 },
}
