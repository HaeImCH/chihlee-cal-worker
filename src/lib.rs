pub mod cache;
pub mod csv_pipeline;
pub mod error;
pub mod models;
pub mod routes;
pub mod source_scraper;

use worker::{Context, Env, Request, Response, Result, ScheduleContext, ScheduledEvent, event};

#[event(fetch)]
async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    routes::handle(req, env, ctx).await
}

#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let source_url = env
        .var("SOURCE_URL")
        .map(|value| value.to_string())
        .unwrap_or_else(|_| models::DEFAULT_SOURCE_URL.to_string());

    if let Err(error) = csv_pipeline::sync_all_semesters(&source_url).await {
        worker::console_error!("scheduled csv sync failed: {error}");
    }
}
