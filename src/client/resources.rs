//! Per-resource methods on `AhaClient`. Lists walk all pages sequentially
//! using Aha!'s `pagination.total_pages` field. Show endpoints are simple
//! GETs. The deep `feature_show` does a bounded-concurrency fan-out to
//! pull requirements + comments + todos in parallel.

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use serde::Deserialize;

use super::models::*;
use super::AhaClient;

const DEFAULT_PER_PAGE: u32 = 200;
/// Aha! caps API at ~5 req/sec. Cap parallel fan-out at 3 to leave headroom
/// for the rest of the request budget. Same value the MCP server uses.
const FANOUT_CONCURRENCY: usize = 3;

/// Generic envelope for paginated list responses.
#[derive(Debug, Deserialize)]
struct ListEnvelope<T> {
    #[serde(default)]
    products: Vec<T>,
    #[serde(default)]
    releases: Vec<T>,
    #[serde(default)]
    epics: Vec<T>,
    #[serde(default)]
    features: Vec<T>,
    #[serde(default)]
    requirements: Vec<T>,
    #[serde(default)]
    tasks: Vec<T>,
    #[serde(default)]
    comments: Vec<T>,
    #[serde(default)]
    ideas: Vec<T>,
    #[serde(default)]
    pagination: Pagination,
}

impl<T> ListEnvelope<T> {
    fn into_items(self, key: ListKey) -> Vec<T> {
        match key {
            ListKey::Products => self.products,
            ListKey::Releases => self.releases,
            ListKey::Epics => self.epics,
            ListKey::Features => self.features,
            ListKey::Requirements => self.requirements,
            ListKey::Tasks => self.tasks,
            ListKey::Comments => self.comments,
            ListKey::Ideas => self.ideas,
        }
    }
}

#[derive(Copy, Clone)]
enum ListKey {
    Products,
    Releases,
    Epics,
    Features,
    Requirements,
    Tasks,
    Comments,
    Ideas,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // only one variant is used per call
struct OneEnvelope<T> {
    #[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
    product: Option<T>,
    #[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
    release: Option<T>,
    #[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
    epic: Option<T>,
    #[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
    feature: Option<T>,
    #[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
    requirement: Option<T>,
    #[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
    task: Option<T>,
    #[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
    idea: Option<T>,
}

impl AhaClient {
    /// Walk all pages and collect items.
    async fn list_all<T>(&self, base_path: &str, query: &str, key: ListKey) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let mut items = Vec::new();
        let mut page = 1u32;
        loop {
            let sep = if base_path.contains('?') { '&' } else { '?' };
            let path = format!(
                "{base_path}{sep}page={page}&per_page={DEFAULT_PER_PAGE}{}{query}",
                if query.is_empty() { "" } else { "&" }
            );
            let env: ListEnvelope<T> = self.get_json(&path).await?;
            let total_pages = env.pagination.total_pages;
            items.extend(env.into_items(key));
            if total_pages <= page {
                break;
            }
            page += 1;
        }
        Ok(items)
    }

    // ---------- Products ----------

    pub async fn list_products(&self) -> Result<Vec<Product>> {
        self.list_all("/products", "include_teams=true", ListKey::Products)
            .await
    }

    // ---------- Releases ----------

    pub async fn list_releases(&self, product_filter: Option<&str>) -> Result<Vec<Release>> {
        let path = match product_filter {
            Some(p) => format!("/products/{p}/releases"),
            None => "/releases".to_string(),
        };
        self.list_all(&path, "", ListKey::Releases).await
    }

    pub async fn get_release(&self, id_or_ref: &str) -> Result<Release> {
        let env: OneEnvelope<Release> = self.get_json(&format!("/releases/{id_or_ref}")).await?;
        env.release
            .ok_or_else(|| anyhow::anyhow!("release {id_or_ref} not found"))
    }

    // ---------- Epics ----------

    pub async fn list_epics(
        &self,
        product_filter: Option<&str>,
        release_filter: Option<&str>,
    ) -> Result<Vec<Epic>> {
        let path = match (product_filter, release_filter) {
            (_, Some(r)) => format!("/releases/{r}/epics"),
            (Some(p), None) => format!("/products/{p}/epics"),
            (None, None) => "/epics".to_string(),
        };
        self.list_all(&path, "", ListKey::Epics).await
    }

    pub async fn get_epic(&self, id_or_ref: &str) -> Result<Epic> {
        let env: OneEnvelope<Epic> = self.get_json(&format!("/epics/{id_or_ref}")).await?;
        env.epic
            .ok_or_else(|| anyhow::anyhow!("epic {id_or_ref} not found"))
    }

    // ---------- Features ----------

    pub async fn list_features(&self, filters: &FeatureFilters) -> Result<Vec<Feature>> {
        let mut path = String::new();
        if let Some(r) = &filters.release {
            path = format!("/releases/{r}/features");
        } else if let Some(e) = &filters.epic {
            path = format!("/epics/{e}/features");
        } else if let Some(p) = &filters.product {
            path = format!("/products/{p}/features");
        } else {
            path.push_str("/features");
        }
        let mut q = Vec::new();
        if let Some(s) = &filters.query {
            q.push(format!("q={}", urlencoding(s)));
        }
        if let Some(s) = &filters.tag {
            q.push(format!("tag={}", urlencoding(s)));
        }
        if let Some(s) = &filters.assigned_to_user {
            q.push(format!("assigned_to_user={}", urlencoding(s)));
        }
        if let Some(s) = &filters.updated_since {
            q.push(format!("updated_since={}", urlencoding(s)));
        }
        let query = q.join("&");
        self.list_all(&path, &query, ListKey::Features).await
    }

    pub async fn get_feature(&self, id_or_ref: &str) -> Result<Feature> {
        let env: OneEnvelope<Feature> = self.get_json(&format!("/features/{id_or_ref}")).await?;
        env.feature
            .ok_or_else(|| anyhow::anyhow!("feature {id_or_ref} not found"))
    }

    /// Deep view: fetch the feature, then in parallel pull requirements,
    /// comments, and the per-todo details (each todo needs an extra GET to
    /// surface body/comments). Bounded at `FANOUT_CONCURRENCY` to stay under
    /// the rate limit.
    pub async fn feature_show(&self, id_or_ref: &str) -> Result<FeatureDeep> {
        let feature = self.get_feature(id_or_ref).await?;
        let key = if feature.reference_num.is_empty() {
            feature.id.clone()
        } else {
            feature.reference_num.clone()
        };
        let requirements_path = format!("/features/{key}/requirements?per_page={DEFAULT_PER_PAGE}");
        let comments_path = format!("/features/{key}/comments?per_page={DEFAULT_PER_PAGE}");
        let tasks_path = format!("/features/{key}/tasks?per_page={DEFAULT_PER_PAGE}");

        let (req_env, com_env, task_env) = tokio::try_join!(
            self.get_json::<ListEnvelope<Requirement>>(&requirements_path),
            self.get_json::<ListEnvelope<Comment>>(&comments_path),
            self.get_json::<ListEnvelope<Todo>>(&tasks_path),
        )?;

        let requirements = req_env.into_items(ListKey::Requirements);
        let comments = com_env.into_items(ListKey::Comments);
        let todos = task_env.into_items(ListKey::Tasks);

        // Fan out per-todo: pull the full task object (`body`, `attachments`,
        // …) plus its comments. The list endpoint above returns lean todos
        // without body/attachments, so a per-id GET is the only way to get
        // them. Bounded so the combined fan-out stays under the rate limit.
        let todos_with_comments: Vec<TodoDeep> = stream::iter(todos)
            .map(|todo| {
                let id = todo.id.clone();
                async move {
                    let task_path = format!("/tasks/{id}");
                    let comments_path = format!("/tasks/{id}/comments");
                    let (full, comments_resp) = tokio::join!(
                        self.get_json::<OneEnvelope<Todo>>(&task_path),
                        self.get_json::<ListEnvelope<Comment>>(&comments_path),
                    );
                    let todo = full.ok().and_then(|e| e.task).unwrap_or(todo);
                    let comments = comments_resp
                        .map(|e| e.into_items(ListKey::Comments))
                        .unwrap_or_default();
                    TodoDeep { todo, comments }
                }
            })
            .buffer_unordered(FANOUT_CONCURRENCY)
            .collect()
            .await;

        Ok(FeatureDeep {
            feature,
            requirements,
            comments,
            todos: todos_with_comments,
        })
    }

    // ---------- Requirements ----------

    pub async fn get_requirement(&self, id_or_ref: &str) -> Result<Requirement> {
        let env: OneEnvelope<Requirement> =
            self.get_json(&format!("/requirements/{id_or_ref}")).await?;
        env.requirement
            .ok_or_else(|| anyhow::anyhow!("requirement {id_or_ref} not found"))
    }

    // ---------- Todos ----------

    pub async fn list_todos(&self, feature_filter: Option<&str>) -> Result<Vec<Todo>> {
        let path = match feature_filter {
            Some(f) => format!("/features/{f}/tasks"),
            None => "/tasks".to_string(),
        };
        self.list_all(&path, "", ListKey::Tasks).await
    }

    pub async fn get_todo(&self, id: &str) -> Result<Todo> {
        let env: OneEnvelope<Todo> = self.get_json(&format!("/tasks/{id}")).await?;
        env.task
            .ok_or_else(|| anyhow::anyhow!("todo {id} not found"))
    }

    /// Deep view: full todo (body + attachments via per-task GET) plus its
    /// comments (each with its own attachments). Same parallel-pair as
    /// the per-todo branch of `feature_show`, exposed standalone so users
    /// can drill into a todo without going through its parent feature.
    pub async fn todo_show(&self, id: &str) -> Result<TodoDeep> {
        let task_path = format!("/tasks/{id}");
        let comments_path = format!("/tasks/{id}/comments");
        let (full, comments_resp) = tokio::join!(
            self.get_json::<OneEnvelope<Todo>>(&task_path),
            self.get_json::<ListEnvelope<Comment>>(&comments_path),
        );
        let todo = full?
            .task
            .ok_or_else(|| anyhow::anyhow!("todo {id} not found"))?;
        let comments = comments_resp
            .map(|e| e.into_items(ListKey::Comments))
            .unwrap_or_default();
        Ok(TodoDeep { todo, comments })
    }

    // ---------- Ideas ----------

    pub async fn list_ideas(&self, product_filter: Option<&str>) -> Result<Vec<Idea>> {
        let path = match product_filter {
            Some(p) => format!("/products/{p}/ideas"),
            None => "/ideas".to_string(),
        };
        self.list_all(&path, "", ListKey::Ideas).await
    }

    pub async fn get_idea(&self, id_or_ref: &str) -> Result<Idea> {
        let env: OneEnvelope<Idea> = self.get_json(&format!("/ideas/{id_or_ref}")).await?;
        env.idea
            .ok_or_else(|| anyhow::anyhow!("idea {id_or_ref} not found"))
    }

    // ---------- Attachments ----------

    /// Look up attachment metadata by id. The response carries a fresh
    /// `download_url` — typically a short-lived presigned URL, so re-fetch
    /// before streaming bytes rather than relying on a cached value.
    pub async fn get_attachment(&self, id: &str) -> Result<Attachment> {
        // Aha! returns `{"attachment": {...}}` here; OneEnvelope doesn't yet
        // model that key, so deserialize the wrapper inline.
        #[derive(serde::Deserialize)]
        struct Wrapper {
            attachment: Attachment,
        }
        let w: Wrapper = self.get_json(&format!("/attachments/{id}")).await?;
        Ok(w.attachment)
    }

    /// Resolve the attachment, then stream bytes from `download_url` into
    /// `writer`. Returns the (refreshed) metadata so callers can name the
    /// output file.
    ///
    /// What we've seen empirically: API tokens can download every
    /// attachment whose `file_size` is set in the metadata. Attachments
    /// where the API returns `file_size: null` come back as 302 →
    /// `/access_denied` → 500 — both for the API and for a logged-in
    /// browser session. The blob appears to be deleted from Aha!'s
    /// storage even though the metadata pointer survives.
    ///
    /// We use a fresh client with redirects disabled so we can detect
    /// the missing-blob case as a clean 302 instead of chasing the URL
    /// into an opaque 500.
    pub async fn download_attachment<W>(&self, id: &str, writer: &mut W) -> Result<Attachment>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt;
        let meta = self.get_attachment(id).await?;

        // Fast-fail before we touch the wire. `file_size: null` on an
        // attachment record reliably means Aha! has the metadata but no
        // longer has the blob — every URL variant 302s to /access_denied
        // for both API tokens and logged-in browser sessions. Telling the
        // user upfront beats a confusing redirect chain.
        if meta.file_size.is_none() {
            anyhow::bail!(
                "attachment {id} ({}) is tombstoned: Aha! still serves the \
                 metadata pointer but reports `file_size: null` and \
                 `original_file_size: null`, which has consistently meant the \
                 blob has been purged from their storage. The bytes are \
                 unrecoverable through any URL we've tested (API token, \
                 browser session, every `?size=` variant — all 302 to \
                 /access_denied). Aha! support may be able to restore from \
                 backup if the file is critical; we can't fetch it from here.",
                meta.file_name,
            );
        }

        let url = meta
            .download_url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("attachment {id} has no download_url"))?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("building download client")?;
        let resp = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        if status.is_redirection() {
            // file_size was non-null but the URL still redirected — outside
            // the tombstoned pattern we've seen. Surface the redirect
            // verbatim so the user can see what Aha! is doing.
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(no location header)");
            anyhow::bail!(
                "attachment {id}: signed download_url returned {status} → \
                 {location}. (Unexpected: file_size was set on the metadata, \
                 so we'd expect this to download. Try opening the URL in a \
                 logged-in Aha! browser tab; if that also fails, please \
                 report the attachment id.)\nURL: {url}"
            );
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("downloading attachment {id}: HTTP {status}: {body}");
        }
        let mut stream = resp.bytes_stream();
        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.context("reading attachment stream")?;
            writer
                .write_all(&bytes)
                .await
                .context("writing attachment bytes")?;
        }
        writer.flush().await.context("flushing attachment writer")?;
        Ok(meta)
    }
}

#[derive(Debug, Default, Clone)]
pub struct FeatureFilters {
    pub product: Option<String>,
    pub release: Option<String>,
    pub epic: Option<String>,
    pub query: Option<String>,
    pub tag: Option<String>,
    pub assigned_to_user: Option<String>,
    pub updated_since: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FeatureDeep {
    pub feature: Feature,
    pub requirements: Vec<Requirement>,
    pub comments: Vec<Comment>,
    pub todos: Vec<TodoDeep>,
}

#[derive(Debug, Clone)]
pub struct TodoDeep {
    pub todo: Todo,
    pub comments: Vec<Comment>,
}

/// Tiny URL encoder for query string values. Only escapes a handful of
/// reserved chars; full RFC 3986 isn't needed for our use.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            ' ' => out.push_str("%20"),
            _ => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Credentials;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn creds() -> Credentials {
        Credentials {
            subdomain: "tcare".into(),
            token: "t".into(),
        }
    }

    #[tokio::test]
    async fn list_products_walks_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/products"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "products": [{"id":"1","reference_prefix":"A","name":"Alpha"}],
                "pagination": {"current_page": 1, "total_pages": 2, "total_records": 2}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/products"))
            .and(query_param("page", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "products": [{"id":"2","reference_prefix":"B","name":"Beta"}],
                "pagination": {"current_page": 2, "total_pages": 2, "total_records": 2}
            })))
            .mount(&server)
            .await;

        let client = AhaClient::with_base_url(&creds(), &server.uri()).unwrap();
        let products = client.list_products().await.unwrap();
        assert_eq!(products.len(), 2);
        assert_eq!(products[0].name, "Alpha");
        assert_eq!(products[1].name, "Beta");
    }

    #[tokio::test]
    async fn feature_show_fans_out_in_parallel() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/features/TC-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "feature": {"id":"100","reference_num":"TC-1","name":"feat"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/features/TC-1/requirements"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "requirements": [{"id":"r1","reference_num":"TC-1-1","name":"req"}],
                "pagination": {"current_page":1,"total_pages":1,"total_records":1}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/features/TC-1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "comments": [{"id":"c1","body":"hi"}],
                "pagination": {"current_page":1,"total_pages":1,"total_records":1}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/features/TC-1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tasks": [{"id":"t1","name":"todo1"}, {"id":"t2","name":"todo2"}],
                "pagination": {"current_page":1,"total_pages":1,"total_records":2}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tasks/t1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "task": {"id":"t1","name":"todo1"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tasks/t2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "task": {"id":"t2","name":"todo2"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tasks/t1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "comments": [],
                "pagination": {"current_page":1,"total_pages":1,"total_records":0}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tasks/t2/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "comments": [{"id":"tc","body":"hi"}],
                "pagination": {"current_page":1,"total_pages":1,"total_records":1}
            })))
            .mount(&server)
            .await;

        let client = AhaClient::with_base_url(&creds(), &server.uri()).unwrap();
        let deep = client.feature_show("TC-1").await.unwrap();
        assert_eq!(deep.feature.reference_num, "TC-1");
        assert_eq!(deep.requirements.len(), 1);
        assert_eq!(deep.comments.len(), 1);
        assert_eq!(deep.todos.len(), 2);
    }

    #[tokio::test]
    async fn feature_show_surfaces_todo_body_and_attachments() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/features/TC-2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "feature": {"id":"200","reference_num":"TC-2","name":"feat"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/features/TC-2/requirements"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "requirements": [],
                "pagination": {"current_page":1,"total_pages":1,"total_records":0}
            })))
            .mount(&server)
            .await;
        // Feature comment with an attachment.
        Mock::given(method("GET"))
            .and(path("/features/TC-2/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "comments": [{
                    "id": "c1",
                    "body": "see screenshot",
                    "attachments": [{
                        "id": "att1",
                        "file_name": "screenshot.png",
                        "download_url": "https://aha.example/files/att1",
                        "content_type": "image/png",
                        "file_size": 12345
                    }]
                }],
                "pagination": {"current_page":1,"total_pages":1,"total_records":1}
            })))
            .mount(&server)
            .await;
        // Lean list response — no body, no attachments.
        Mock::given(method("GET"))
            .and(path("/features/TC-2/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tasks": [{"id":"t1","name":"Investigate bug"}],
                "pagination": {"current_page":1,"total_pages":1,"total_records":1}
            })))
            .mount(&server)
            .await;
        // Per-task GET surfaces body + attachments.
        Mock::given(method("GET"))
            .and(path("/tasks/t1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "task": {
                    "id": "t1",
                    "name": "Investigate bug",
                    "body": "Reproduce with the attached log.",
                    "attachments": [{
                        "id": "att2",
                        "file_name": "trace.log",
                        "download_url": "https://aha.example/files/att2",
                        "content_type": "text/plain",
                        "file_size": 9001
                    }]
                }
            })))
            .mount(&server)
            .await;
        // Todo comment with its own attachment.
        Mock::given(method("GET"))
            .and(path("/tasks/t1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "comments": [{
                    "id": "tc1",
                    "body": "follow-up",
                    "attachments": [{
                        "id": "att3",
                        "file_name": "diff.patch",
                        "content_type": "text/x-diff",
                        "file_size": 256
                    }]
                }],
                "pagination": {"current_page":1,"total_pages":1,"total_records":1}
            })))
            .mount(&server)
            .await;

        let client = AhaClient::with_base_url(&creds(), &server.uri()).unwrap();
        let deep = client.feature_show("TC-2").await.unwrap();

        // Feature-level comment attachment.
        assert_eq!(deep.comments[0].attachments.len(), 1);
        assert_eq!(deep.comments[0].attachments[0].file_name, "screenshot.png");
        assert_eq!(
            deep.comments[0].attachments[0].content_type.as_deref(),
            Some("image/png")
        );
        assert_eq!(deep.comments[0].attachments[0].file_size, Some(12345));

        // Todo body + attachment came from the per-task GET, not the list.
        assert_eq!(deep.todos.len(), 1);
        let td = &deep.todos[0];
        assert_eq!(
            td.todo.body.as_deref(),
            Some("Reproduce with the attached log.")
        );
        assert_eq!(td.todo.attachments.len(), 1);
        assert_eq!(td.todo.attachments[0].file_name, "trace.log");

        // Todo-comment attachment.
        assert_eq!(td.comments.len(), 1);
        assert_eq!(td.comments[0].attachments.len(), 1);
        assert_eq!(td.comments[0].attachments[0].file_name, "diff.patch");
        // download_url omitted in fixture — should default to None, not error.
        assert!(td.comments[0].attachments[0].download_url.is_none());
    }

    #[test]
    fn urlencoding_escapes_reserved() {
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("foo=bar"), "foo%3Dbar");
        assert_eq!(urlencoding("simple"), "simple");
    }
}
