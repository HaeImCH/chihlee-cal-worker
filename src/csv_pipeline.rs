use chihlee_cal_to_csv::{ExtractOptions, extract_pdf_bytes_to_csv_string};
use url::Url;
use worker::Fetch;

use crate::cache;
use crate::error::ApiError;
use crate::models::SemesterLink;
use crate::source_scraper;

pub const CSV_CACHE_TTL_SECONDS: u32 = 120 * 24 * 60 * 60;
pub const CSV_CACHE_KEY_PREFIX: &str = "csv:semester:v1:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvCacheStatus {
    Hit,
    Miss,
    Bypass,
}

impl CsvCacheStatus {
    pub const fn as_header_value(self) -> &'static str {
        match self {
            Self::Hit => "HIT",
            Self::Miss => "MISS",
            Self::Bypass => "BYPASS",
        }
    }
}

pub fn csv_cache_key(semester: i32) -> String {
    format!("{CSV_CACHE_KEY_PREFIX}{semester}")
}

pub async fn get_or_build_csv_for_link(link: &SemesterLink) -> Result<String, ApiError> {
    let (csv, _) = get_or_build_csv_for_link_with_status(link).await?;
    Ok(csv)
}

pub async fn get_or_build_csv_for_link_with_status(
    link: &SemesterLink,
) -> Result<(String, CsvCacheStatus), ApiError> {
    let cache_key = csv_cache_key(link.semester);
    if let Some(cached) = cache::get_bytes(&cache_key).await? {
        let csv = String::from_utf8(cached).map_err(|error| {
            ApiError::Internal(format!("cached csv is not valid UTF-8: {error}"))
        })?;
        return Ok((csv, CsvCacheStatus::Hit));
    }

    let csv = build_csv_from_pdf_url(&link.url).await?;
    put_csv_in_cache(link.semester, &csv).await?;
    Ok((csv, CsvCacheStatus::Miss))
}

pub async fn rebuild_csv_for_link(link: &SemesterLink) -> Result<String, ApiError> {
    let (csv, _) = rebuild_csv_for_link_with_status(link).await?;
    Ok(csv)
}

pub async fn rebuild_csv_for_link_with_status(
    link: &SemesterLink,
) -> Result<(String, CsvCacheStatus), ApiError> {
    let csv = build_csv_from_pdf_url(&link.url).await?;
    put_csv_in_cache(link.semester, &csv).await?;
    Ok((csv, CsvCacheStatus::Bypass))
}

async fn put_csv_in_cache(semester: i32, csv: &str) -> Result<(), ApiError> {
    cache::put_bytes(
        &csv_cache_key(semester),
        csv.as_bytes(),
        CSV_CACHE_TTL_SECONDS,
        "text/csv; charset=utf-8",
    )
    .await
}

pub async fn sync_all_semesters(source_url: &str) -> Result<(), ApiError> {
    let links = source_scraper::fetch_semester_links(source_url).await?;
    if links.is_empty() {
        return Err(ApiError::NotFound(
            "no semester PDF links found from source page".to_string(),
        ));
    }

    for link in links {
        if let Err(error) = refresh_csv_for_link(&link).await {
            worker::console_error!(
                "csv sync failed for semester {} ({}): {}",
                link.semester,
                link.url,
                error
            );
        }
    }

    Ok(())
}

async fn refresh_csv_for_link(link: &SemesterLink) -> Result<(), ApiError> {
    let csv = build_csv_from_pdf_url(&link.url).await?;
    put_csv_in_cache(link.semester, &csv).await
}

async fn build_csv_from_pdf_url(pdf_url: &str) -> Result<String, ApiError> {
    let pdf_bytes = fetch_pdf_bytes(pdf_url).await?;
    convert_pdf_bytes_to_csv(&pdf_bytes)
}

async fn fetch_pdf_bytes(pdf_url: &str) -> Result<Vec<u8>, ApiError> {
    let parsed = Url::parse(pdf_url)?;
    let mut response = Fetch::Url(parsed).send().await?;
    let status = response.status_code();
    if status >= 400 {
        return Err(ApiError::Upstream(format!(
            "failed to fetch PDF source: status {status}"
        )));
    }

    let bytes = response.bytes().await?;
    if bytes.is_empty() {
        return Err(ApiError::Upstream("fetched PDF is empty".to_string()));
    }
    Ok(bytes)
}

fn convert_pdf_bytes_to_csv(pdf_bytes: &[u8]) -> Result<String, ApiError> {
    let options = ExtractOptions {
        clean_calendar: true,
        no_page: true,
        no_table: true,
        custom_col_names: Some(("date".to_string(), "event".to_string())),
        ..ExtractOptions::default()
    };

    let (csv, report) = extract_pdf_bytes_to_csv_string(pdf_bytes, &options).map_err(|error| {
        ApiError::Parse(format!(
            "failed to convert PDF using chihlee-cal-to-csv: {error}"
        ))
    })?;

    worker::console_log!(
        "calendar extraction completed: rows={}, tables={}",
        report.row_count,
        report.table_count
    );

    Ok(csv)
}
