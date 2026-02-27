use std::collections::HashSet;

use regex::Regex;
use url::Url;
use worker::Fetch;

use crate::error::ApiError;
use crate::models::SemesterLink;

pub async fn fetch_semester_links(source_url: &str) -> Result<Vec<SemesterLink>, ApiError> {
    let source = Url::parse(source_url)?;
    let mut response = Fetch::Url(source).send().await?;
    let status = response.status_code();
    if status >= 400 {
        return Err(ApiError::Upstream(format!(
            "failed to fetch source page: status {status}"
        )));
    }

    let html = response.text().await?;
    extract_semester_links(&html, source_url)
}

pub fn extract_semester_links(html: &str, source_url: &str) -> Result<Vec<SemesterLink>, ApiError> {
    let base_url = Url::parse(source_url)?;
    let anchor_re = Regex::new(
        r#"(?is)<a[^>]*href\s*=\s*["'](?P<href>[^"'#>]+\.pdf(?:\?[^"'#>]*)?)["'][^>]*>(?P<text>.*?)</a>"#,
    )
    .map_err(|error| ApiError::Internal(error.to_string()))?;

    let mut seen = HashSet::new();
    let mut links = Vec::new();

    for capture in anchor_re.captures_iter(html) {
        let Some(href_match) = capture.name("href") else {
            continue;
        };
        let href = href_match.as_str().trim();
        let joined_url = match base_url.join(href) {
            Ok(url) => url,
            Err(_) => continue,
        };

        let raw_text = capture
            .name("text")
            .map(|value| value.as_str())
            .unwrap_or_default();
        let clean_text = strip_html_tags(raw_text).trim().to_string();

        let semester = extract_semester(raw_text)
            .or_else(|| extract_semester(href))
            .or_else(|| extract_semester(joined_url.path()))
            .unwrap_or(-1);

        if semester < 0 {
            continue;
        }

        if seen.insert(semester) {
            links.push(SemesterLink {
                semester,
                url: joined_url.to_string(),
                title: clean_text,
            });
        }
    }

    links.sort_by(|left, right| right.semester.cmp(&left.semester));
    Ok(links)
}

pub fn extract_semester(input: &str) -> Option<i32> {
    let decoded = urlencoding::decode(input)
        .map(std::borrow::Cow::into_owned)
        .unwrap_or_else(|_| input.to_string());
    let semester_re = Regex::new(r"(?:^|\D)(\d{3})(?:\D|$)").ok()?;
    semester_re
        .captures(&decoded)
        .and_then(|capture| capture.get(1))
        .and_then(|value| value.as_str().parse::<i32>().ok())
}

fn strip_html_tags(input: &str) -> String {
    let tags_re = Regex::new(r"(?is)<[^>]+>").expect("hardcoded HTML tags regex is valid");
    tags_re.replace_all(input, " ").to_string()
}
