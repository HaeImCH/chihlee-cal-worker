use serde::{Deserialize, Serialize};

pub const DEFAULT_SOURCE_URL: &str = "https://www.chihlee.edu.tw/p/404-1000-62149.php";
pub const LINKS_CACHE_KEY: &str = "cal:links:v1";
pub const LINKS_CACHE_TTL_SECONDS: u32 = 6 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemesterLink {
    pub semester: i32,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedBy {
    Current,
    Latest,
    Explicit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CurrentSemesterResponse {
    pub semester: i32,
    pub roc_year: i32,
    pub latest_available: i32,
    pub source_url: String,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalLinkSingleResponse {
    pub semester: i32,
    pub url: String,
    pub resolved_by: ResolvedBy,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalLinkAllResponse {
    pub items: Vec<SemesterLink>,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}
