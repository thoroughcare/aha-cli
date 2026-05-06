//! 429 retry with `Retry-After` honoring. Aha! caps the API at ~5 req/sec
//! and returns 429 with a `Retry-After` header in seconds. We sleep for the
//! requested duration (or fall back to exponential backoff if the header is
//! missing) and retry up to `max_retries` times.

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Method, Request, RequestBuilder, Response, StatusCode};

use super::AhaClient;

const MAX_RETRIES: u32 = 3;
const FALLBACK_BACKOFF_MS: u64 = 500;

impl AhaClient {
    /// GET `<base_url><path>` and decode JSON. Retries on 429 with backoff.
    pub(super) async fn get_json<T>(&self, path: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url(), path);
        let resp = self.send_with_retry(self.http().get(&url)).await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GET {url} returned {status}: {body}");
        }
        resp.json().await.with_context(|| format!("decoding {url}"))
    }

    pub(super) async fn send_with_retry(&self, builder: RequestBuilder) -> Result<Response> {
        let request = builder.build().context("building request")?;
        for attempt in 0..=MAX_RETRIES {
            let cloned = clone_request(&request)?;
            let resp = self.http().execute(cloned).await.context("HTTP send")?;
            if resp.status() != StatusCode::TOO_MANY_REQUESTS || attempt == MAX_RETRIES {
                return Ok(resp);
            }
            let wait = retry_after(&resp).unwrap_or_else(|| backoff_for(attempt));
            tokio::time::sleep(wait).await;
        }
        unreachable!("loop exits via early return on every iteration");
    }
}

fn retry_after(resp: &Response) -> Option<Duration> {
    let header = resp.headers().get(reqwest::header::RETRY_AFTER)?;
    let s = header.to_str().ok()?.trim();
    let secs: f64 = s.parse().ok()?;
    if secs.is_finite() && secs >= 0.0 {
        Some(Duration::from_millis((secs * 1000.0) as u64))
    } else {
        None
    }
}

fn backoff_for(attempt: u32) -> Duration {
    Duration::from_millis(FALLBACK_BACKOFF_MS * (1u64 << attempt))
}

/// reqwest `Request` is not `Clone` (the body might be a stream). For our
/// JSON GETs the body is always `None`, so a manual shallow clone is safe.
fn clone_request(src: &Request) -> Result<Request> {
    let method: Method = src.method().clone();
    let url = src.url().clone();
    let mut new = Request::new(method, url);
    *new.headers_mut() = src.headers().clone();
    *new.timeout_mut() = src.timeout().copied();
    Ok(new)
}
