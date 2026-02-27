use serde::{Serialize, de::DeserializeOwned};
use worker::{Cache, Response};

use crate::error::ApiError;

fn cache_url(key: &str) -> String {
    format!("https://cache.local/{}", urlencoding::encode(key))
}

pub async fn get_json<T>(key: &str) -> Result<Option<T>, ApiError>
where
    T: DeserializeOwned,
{
    let cache = Cache::default();
    let mut cached = cache.get(cache_url(key), true).await?;

    let Some(mut response) = cached.take() else {
        return Ok(None);
    };

    let body = response.text().await?;
    let parsed = serde_json::from_str::<T>(&body)?;
    Ok(Some(parsed))
}

pub async fn put_json<T>(key: &str, value: &T, ttl_seconds: u32) -> Result<(), ApiError>
where
    T: Serialize,
{
    let cache = Cache::default();
    let body = serde_json::to_string(value)?;

    let mut response = Response::ok(body)?;
    response
        .headers_mut()
        .set("Cache-Control", &format!("public, max-age={ttl_seconds}"))?;
    response
        .headers_mut()
        .set("Content-Type", "application/json; charset=utf-8")?;

    cache.put(cache_url(key), response).await?;
    Ok(())
}

pub async fn get_bytes(key: &str) -> Result<Option<Vec<u8>>, ApiError> {
    let cache = Cache::default();
    let mut cached = cache.get(cache_url(key), true).await?;

    let Some(mut response) = cached.take() else {
        return Ok(None);
    };

    let payload = response.bytes().await?;
    Ok(Some(payload))
}

pub async fn put_bytes(
    key: &str,
    bytes: &[u8],
    ttl_seconds: u32,
    content_type: &str,
) -> Result<(), ApiError> {
    let cache = Cache::default();
    let mut response = Response::from_bytes(bytes.to_vec())?;
    response
        .headers_mut()
        .set("Cache-Control", &format!("public, max-age={ttl_seconds}"))?;
    response.headers_mut().set("Content-Type", content_type)?;

    cache.put(cache_url(key), response).await?;
    Ok(())
}
