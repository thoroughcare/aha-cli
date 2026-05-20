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

    /// Public-API variant of `get_json` for examples / probes that want
    /// raw JSON without going through our typed model.
    pub async fn get_json_raw(&self, path: &str) -> Result<serde_json::Value> {
        self.get_json(path).await
    }

    /// POST a JSON body to `<base_url><path>` and decode the response. No
    /// retry on 429: POSTs aren't idempotent — a 429 after a successful but
    /// slow create could double-write. We surface the error instead.
    pub(super) async fn post_json<B, T>(&self, path: &str, body: &B) -> Result<T>
    where
        B: serde::Serialize + ?Sized,
        T: serde::de::DeserializeOwned,
    {
        self.send_json_request(Method::POST, path, body).await
    }

    /// PUT a JSON body to `<base_url><path>` and decode the response. PUT is
    /// idempotent (replace-style on Aha!) so it goes through the 429 retry
    /// loop alongside GET.
    pub(super) async fn put_json<B, T>(&self, path: &str, body: &B) -> Result<T>
    where
        B: serde::Serialize + ?Sized,
        T: serde::de::DeserializeOwned,
    {
        self.send_json_request(Method::PUT, path, body).await
    }

    async fn send_json_request<B, T>(&self, method: Method, path: &str, body: &B) -> Result<T>
    where
        B: serde::Serialize + ?Sized,
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url(), path);
        // Serialize the body once. We hold onto the bytes so PUT can retry
        // on 429 by reconstructing an identical request — reqwest's
        // `Body::try_clone` is `pub(crate)`, so we can't lean on that.
        let bytes = serde_json::to_vec(body).context("serializing request body")?;
        let build = || {
            self.http()
                .request(method.clone(), &url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(bytes.clone())
        };

        let resp = if method == Method::POST {
            // No retry middleware on POST: a 429 after a slow-but-successful
            // create could double-write. Surface the error instead.
            build().send().await.context("HTTP send")?
        } else {
            self.send_with_retry_resend(method.clone(), &url, &bytes)
                .await?
        };

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            // 422 carries Aha!'s validation messages — surface verbatim so
            // users see exactly which field was rejected.
            if status.as_u16() == 422 {
                anyhow::bail!("Aha! rejected the request (422): {body}");
            }
            if status.as_u16() == 404 {
                anyhow::bail!("not found: {path} — check the reference or id");
            }
            anyhow::bail!("{method} {url} returned {status}: {body}");
        }
        resp.json().await.with_context(|| format!("decoding {url}"))
    }

    /// Retry loop for requests whose body is held as raw bytes — we rebuild
    /// the request from scratch each attempt rather than cloning a `Request`
    /// (reqwest's `Body::try_clone` is not in the public API).
    async fn send_with_retry_resend(
        &self,
        method: Method,
        url: &str,
        bytes: &[u8],
    ) -> Result<Response> {
        for attempt in 0..=MAX_RETRIES {
            let resp = self
                .http()
                .request(method.clone(), url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(bytes.to_vec())
                .send()
                .await
                .context("HTTP send")?;
            if resp.status() != StatusCode::TOO_MANY_REQUESTS || attempt == MAX_RETRIES {
                return Ok(resp);
            }
            let wait = retry_after(&resp).unwrap_or_else(|| backoff_for(attempt));
            tokio::time::sleep(wait).await;
        }
        unreachable!("loop exits via early return on every iteration");
    }

    pub(super) async fn send_with_retry(&self, builder: RequestBuilder) -> Result<Response> {
        let request = builder.build().context("building request")?;
        debug_assert!(
            request.method() != Method::POST,
            "send_with_retry must not be called for POST — see post_json"
        );
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
