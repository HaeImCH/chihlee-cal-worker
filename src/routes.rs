use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, Utc};
use serde::Serialize;
use worker::{Context, Env, Request, Response, Result, RouteContext, Router};

use crate::cache;
use crate::csv_pipeline;
use crate::error::ApiError;
use crate::models::{
    CalLinkAllResponse, CalLinkSingleResponse, CurrentSemesterResponse, LINKS_CACHE_KEY,
    LINKS_CACHE_TTL_SECONDS, ResolvedBy, SemesterLink,
};
use crate::source_scraper;

#[derive(Debug, Clone)]
pub struct AppState {
    pub source_url: String,
}

pub async fn handle(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let source_url = env
        .var("SOURCE_URL")
        .map(|value| value.to_string())
        .unwrap_or_else(|_| crate::models::DEFAULT_SOURCE_URL.to_string());

    let state = AppState { source_url };

    Router::with_data(state)
        .get_async("/api/v1/current_semester", current_semester_route)
        .get_async("/api/v1/cal_link", cal_link_route)
        .get_async("/api/v1/csv", csv_route)
        .run(req, env)
        .await
}

async fn current_semester_route(_req: Request, ctx: RouteContext<AppState>) -> Result<Response> {
    match current_semester_response(&ctx.data.source_url).await {
        Ok(response) => json_response(&response),
        Err(error) => error.into_response(),
    }
}

async fn cal_link_route(req: Request, ctx: RouteContext<AppState>) -> Result<Response> {
    match cal_link_response(&req, &ctx.data.source_url).await {
        Ok(response) => json_response(&response),
        Err(error) => error.into_response(),
    }
}

async fn csv_route(req: Request, ctx: RouteContext<AppState>) -> Result<Response> {
    match csv_response(&req, &ctx.data.source_url).await {
        Ok(response) => Ok(response),
        Err(error) => error.into_response(),
    }
}

async fn current_semester_response(source_url: &str) -> Result<CurrentSemesterResponse, ApiError> {
    let (links, cached) = load_links(source_url).await?;
    let latest_available = latest_semester(&links)?;
    let roc_year = current_roc_year_now();
    let target = roc_year - 1;
    let semester = resolve_current_semester(target, &links);

    Ok(CurrentSemesterResponse {
        semester,
        roc_year,
        target,
        latest_available,
        source_url: source_url.to_string(),
        cached,
    })
}

async fn cal_link_response(
    req: &Request,
    source_url: &str,
) -> Result<CalLinkResponseEnvelope, ApiError> {
    let query = parse_query(req)?;
    let semester_param = parse_semester_query(&query)?;
    let all = parse_all_query(&query);

    let (links, cached) = load_links(source_url).await?;

    if all {
        return Ok(CalLinkResponseEnvelope::All(CalLinkAllResponse {
            items: links,
            cached,
        }));
    }

    let roc_year = current_roc_year_now();
    let selected = resolve_selected_semester(semester_param, &links, roc_year)?;
    let link = find_link(&links, selected.semester)
        .ok_or_else(|| ApiError::NotFound("requested semester link not found".to_string()))?;

    Ok(CalLinkResponseEnvelope::Single(CalLinkSingleResponse {
        semester: link.semester,
        url: link.url.clone(),
        resolved_by: selected.resolved_by,
        cached,
    }))
}

async fn csv_response(req: &Request, source_url: &str) -> Result<Response, ApiError> {
    let query = parse_query(req)?;
    let semester_param = parse_semester_query(&query)?;
    let force = parse_force_query(&query);
    let (links, _) = load_links(source_url).await?;
    let roc_year = current_roc_year_now();
    let selected = resolve_selected_semester(semester_param, &links, roc_year)?;
    let link = find_link(&links, selected.semester)
        .ok_or_else(|| ApiError::NotFound("requested semester link not found".to_string()))?;

    let (csv, cache_status) = if force {
        csv_pipeline::rebuild_csv_for_link_with_status(link).await?
    } else {
        csv_pipeline::get_or_build_csv_for_link_with_status(link).await?
    };
    let mut response = Response::ok(csv)?;
    response
        .headers_mut()
        .set("Content-Type", "text/csv; charset=utf-8")?;
    response.headers_mut().set(
        "Content-Disposition",
        &format!(
            "inline; filename=\"chihlee-calendar-{}.csv\"",
            link.semester
        ),
    )?;
    response
        .headers_mut()
        .set("X-Cache-Status", cache_status.as_header_value())?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response)
}

async fn load_links(source_url: &str) -> Result<(Vec<SemesterLink>, bool), ApiError> {
    if let Some(cached) = cache::get_json::<Vec<SemesterLink>>(LINKS_CACHE_KEY).await? {
        if cached.is_empty() {
            return Err(ApiError::NotFound(
                "no semester PDF links found in cache".to_string(),
            ));
        }
        return Ok((cached, true));
    }

    let links = source_scraper::fetch_semester_links(source_url).await?;
    if links.is_empty() {
        return Err(ApiError::NotFound(
            "no semester PDF links found from source page".to_string(),
        ));
    }

    cache::put_json(LINKS_CACHE_KEY, &links, LINKS_CACHE_TTL_SECONDS).await?;
    Ok((links, false))
}

fn json_response<T>(payload: &T) -> Result<Response>
where
    T: Serialize,
{
    let mut response = Response::from_json(payload)?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response)
}

fn parse_query(req: &Request) -> Result<HashMap<String, String>, ApiError> {
    let url = req.url()?;
    let query = url
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<HashMap<_, _>>();
    Ok(query)
}

fn parse_semester_query(query: &HashMap<String, String>) -> Result<Option<i32>, ApiError> {
    let Some(raw) = query.get("semester") else {
        return Ok(None);
    };

    let parsed = raw.parse::<i32>()?;
    if !(0..=999).contains(&parsed) {
        return Err(ApiError::BadRequest(
            "semester must be within 0..=999".to_string(),
        ));
    }

    Ok(Some(parsed))
}

fn parse_all_query(query: &HashMap<String, String>) -> bool {
    query.get("all").is_some_and(|value| {
        let lowered = value.trim().to_ascii_lowercase();
        lowered == "true" || lowered == "1" || lowered == "yes"
    })
}

fn parse_force_query(query: &HashMap<String, String>) -> bool {
    query.get("force").is_some_and(|value| {
        let lowered = value.trim().to_ascii_lowercase();
        lowered == "true" || lowered == "1" || lowered == "yes"
    })
}

pub fn roc_year_from_utc(now: DateTime<Utc>) -> i32 {
    let taipei_now = now + Duration::hours(8);
    taipei_now.year() - 1911
}

fn current_roc_year_now() -> i32 {
    roc_year_from_utc(Utc::now())
}

pub fn resolve_current_semester(target: i32, links: &[SemesterLink]) -> i32 {
    if links.iter().any(|link| link.semester == target) {
        target
    } else {
        -1
    }
}

pub fn latest_semester(links: &[SemesterLink]) -> Result<i32, ApiError> {
    links
        .first()
        .map(|link| link.semester)
        .ok_or_else(|| ApiError::NotFound("no semester links available".to_string()))
}

pub fn resolve_selected_semester(
    explicit_semester: Option<i32>,
    links: &[SemesterLink],
    roc_year: i32,
) -> Result<SelectedSemester, ApiError> {
    if links.is_empty() {
        return Err(ApiError::NotFound(
            "no semester links available".to_string(),
        ));
    }

    if let Some(semester) = explicit_semester {
        return Ok(SelectedSemester {
            semester,
            resolved_by: ResolvedBy::Explicit,
        });
    }

    let target = roc_year - 1;
    let current_semester = resolve_current_semester(target, links);
    if current_semester >= 0 {
        return Ok(SelectedSemester {
            semester: current_semester,
            resolved_by: ResolvedBy::Current,
        });
    }

    Ok(SelectedSemester {
        semester: latest_semester(links)?,
        resolved_by: ResolvedBy::Latest,
    })
}

fn find_link(links: &[SemesterLink], semester: i32) -> Option<&SemesterLink> {
    links.iter().find(|link| link.semester == semester)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSemester {
    pub semester: i32,
    pub resolved_by: ResolvedBy,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum CalLinkResponseEnvelope {
    Single(CalLinkSingleResponse),
    All(CalLinkAllResponse),
}
