use anyhow::{Context, Result};
use serde::Deserialize;

use crate::auth::Credentials;

/// Minimal Aha! HTTP client. Phase 0.5 only exposes `me()` for `auth check`
/// and `auth whoami`. Phase 1 will extend this with retry middleware,
/// pagination, and resource methods.
#[derive(Debug, Clone)]
pub struct AhaClient {
    base_url: String,
    http: reqwest::Client,
}

impl AhaClient {
    pub fn new(creds: &Credentials) -> Result<Self> {
        // `AHA_BASE_URL` is an undocumented test/debug escape hatch — it
        // lets integration tests point the CLI at a wiremock server. Not
        // surfaced in --help.
        let base = std::env::var("AHA_BASE_URL").unwrap_or_else(|_| creds.base_url());
        Self::with_base_url(creds, &base)
    }

    /// Construct a client pointed at an arbitrary base URL. Used by tests.
    pub fn with_base_url(creds: &Credentials, base_url: &str) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        let mut auth_value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", creds.token))
                .context("invalid token (non-ASCII?)")?;
        auth_value.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, auth_value);
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(concat!("aha-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building HTTP client")?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        })
    }

    /// GET /api/v1/me — used by `auth check` and `auth whoami`.
    pub async fn me(&self) -> Result<MeUser> {
        let url = format!("{}/me", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GET /me returned {status}: {body}");
        }
        let body: MeResponse = resp.json().await.context("decoding /me response")?;
        Ok(body.user)
    }
}

#[derive(Debug, Deserialize)]
struct MeResponse {
    user: MeUser,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeUser {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn creds(token: &str) -> Credentials {
        Credentials {
            subdomain: "tcare".into(),
            token: token.into(),
        }
    }

    #[tokio::test]
    async fn me_sends_bearer_and_decodes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .and(header("authorization", "Bearer testtoken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "user": {
                    "id": "7626760672407598886",
                    "name": "Test User",
                    "email": "test-user@example.com",
                }
            })))
            .mount(&server)
            .await;

        let client = AhaClient::with_base_url(&creds("testtoken"), &server.uri()).unwrap();
        let me = client.me().await.unwrap();
        assert_eq!(me.id, "7626760672407598886"); // 19-digit snowflake stays a string
        assert_eq!(me.email, "test-user@example.com");
    }

    #[tokio::test]
    async fn me_returns_error_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
            .mount(&server)
            .await;

        let client = AhaClient::with_base_url(&creds("bad"), &server.uri()).unwrap();
        let err = client.me().await.unwrap_err();
        assert!(format!("{err:#}").contains("401"));
    }
}
