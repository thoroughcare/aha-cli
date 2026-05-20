# OAuth login — implementation plan

`aha auth login --with-token` already works and is the supported path
today. The browser-based `aha auth login` flow is not yet implemented.
This document is the implementation plan for when we pick it back up —
keep it accurate as we work on it.

## Goal

Run a standard OAuth 2.0 Authorization Code + PKCE flow against
Aha!'s auth server so users can log in by clicking through a browser
prompt, no token paste required. End state matches `gh auth login --web`
and `flyctl auth login`.

After `aha auth login` returns, the same netrc entry shape we already
write today is on disk (`machine <subdomain>.aha.io login oauth password
<token>`), so the rest of the CLI sees no change.

## Prerequisites — must happen before any code merges

1. **Register the OAuth application** on the ThoroughCare Aha!
   account. Admin UI → Account settings → Integrations → Developer →
   Generate OAuth application. Set:
   - **Name**: `aha-cli` (or whatever we want the consent screen to show)
   - **Redirect URI**: `http://127.0.0.1` (loopback; Aha! tolerates a
     dynamic port on loopback redirects per RFC 8252)
   - **Scopes**: read access at minimum; choose whatever matches the
     read-only surface the CLI exposes today
   - **Client type**: public client (no `client_secret`, PKCE-only)

2. Record the resulting `client_id`. This gets baked into the binary at
   build time. No client_secret needed because PKCE makes the public
   client safe.

## High-level flow

```
              ┌──────────────────────────┐
              │ aha auth login --subdomain tcare
              └────────────────┬─────────┘
                               │
        ┌──────────────────────▼────────────────────────┐
        │ 1. bind 127.0.0.1:0 → random port             │
        │ 2. generate PKCE verifier + S256 challenge    │
        │ 3. generate random `state`                    │
        │ 4. build authorize URL                        │
        │ 5. webbrowser::open(authorize_url)            │
        │ 6. spawn_blocking → accept() the callback     │
        └──────────────────────┬────────────────────────┘
                               │      user signs in + authorizes
                               ▼
        Aha! redirects to http://127.0.0.1:<port>/callback?code=…&state=…
                               │
        ┌──────────────────────▼────────────────────────┐
        │ 7. parse query: code, state                   │
        │ 8. validate state matches what we sent        │
        │ 9. respond with "you can close this tab" HTML │
        │ 10. POST /oauth/token, exchange code+verifier │
        │ 11. read access_token from JSON               │
        │ 12. verify token by hitting /api/v1/me        │
        │ 13. write ~/.netrc                            │
        └───────────────────────────────────────────────┘
```

## Module layout

New file: `src/auth/oauth.rs` (~250 LOC). Public surface:

```rust
pub const DEFAULT_AUTH_SERVER: &str = "https://secure.aha.io";

pub struct OAuthConfig {
    pub client_id: String,
    pub auth_server: String,
}

impl OAuthConfig {
    pub fn from_env() -> anyhow::Result<Self>;
}

pub async fn login_flow(subdomain: &str, config: &OAuthConfig) -> anyhow::Result<String>;
```

Private helpers cover: PKCE verifier (32 bytes via `getrandom`,
base64url no-pad), S256 challenge (`sha2::Sha256` of the verifier,
base64url no-pad), the loopback TCP accept loop, HTML response writer,
token exchange.

Re-export the module from `src/auth/mod.rs`:

```rust
pub mod oauth;
```

`src/cli.rs::auth_login` branches:

```rust
let token = if args.with_token {
    // existing stdin path
} else {
    let cfg = crate::auth::oauth::OAuthConfig::from_env()?;
    crate::auth::oauth::login_flow(&subdomain, &cfg).await?
};
```

## Configuration

`client_id` resolution, first hit wins:

1. **`AHA_OAUTH_CLIENT_ID`** env var at runtime — handy for testing
   against alternate apps without rebuilding.
2. **`option_env!("AHA_OAUTH_CLIENT_ID")`** at build time — the
   production path. Built like `AHA_OAUTH_CLIENT_ID=<id> cargo build
   --release` (and in the release workflow once we have one).
3. **Empty** — fall through to a clear error that tells the user to
   either register the OAuth app, set the env var, or rebuild with it
   baked in. Points at `--with-token` as the working fallback.

`AHA_OAUTH_AUTH_SERVER` env var overrides `secure.aha.io` for testing.

## Dependencies to add

| Crate | Why |
|---|---|
| `base64` ^0.22 | URL-safe no-pad encoding for PKCE + state |
| `getrandom` ^0.2 | Crypto-secure RNG for verifier + state |
| `sha2` ^0.10 | S256 PKCE challenge |
| `webbrowser` ^1 | Cross-platform `open <url>` |

All transitively available except `webbrowser` — these are tiny crates
with healthy ecosystems. We deliberately do **not** pull in the
`oauth2` crate; the hand-rolled flow is ~250 LOC and easier to debug.

`tokio` already has `io-std` and `rt-multi-thread`; the OAuth flow uses
`tokio::task::spawn_blocking` to bridge the synchronous
`TcpListener::accept()` into the async runtime, which works with the
features we already have.

## Authorize URL

```
https://secure.aha.io/oauth/authorize?
  client_id=<client_id>&
  redirect_uri=http://127.0.0.1:<port>/callback&
  response_type=code&
  code_challenge=<S256-challenge>&
  code_challenge_method=S256&
  state=<random>&
  subdomain=<user-supplied>
```

The `subdomain` query param is non-standard but Aha!'s consent screen
uses it to scope the workspace hint.

## Token endpoint

```
POST https://secure.aha.io/oauth/token
Content-Type: application/x-www-form-urlencoded

grant_type=authorization_code&
code=<from callback>&
code_verifier=<our verifier>&
client_id=<client_id>&
redirect_uri=http://127.0.0.1:<port>/callback
```

Response: JSON `{ access_token, token_type, … }`. We only need
`access_token`; refresh tokens aren't required for v0.1 (the CLI is
interactive and re-login is cheap).

## Callback server

- `TcpListener::bind("127.0.0.1:0")`, read the port back, build the
  redirect_uri.
- `set_nonblocking(false)` then `accept()` inside `spawn_blocking`.
- Apply a 5-minute read timeout so a user who walks away doesn't pin
  the process forever. (`Ctrl-C` is the documented escape.)
- Parse the request line for `/callback?code=…&state=…`. Drain headers
  to avoid `Connection: close` issues, then write back a small HTML
  "you can close this tab" page so the user gets visual feedback.
- On error (`?error=…`), surface Aha!'s reason and bail.
- On state mismatch, bail with a CSRF warning — never accept the code.

## Testing

The flow is heavily I/O, but we can drive every step without spawning
a real browser. Add to `Cargo.toml`:

```toml
# dev-dependencies already include wiremock — reuse it.
```

### Unit tests in `src/auth/oauth.rs`

- `pkce_verifier_is_43_chars_url_safe()` — RFC 7636 length + alphabet.
- `challenge_is_deterministic_sha256_of_verifier()` — uses the RFC 7636
  example values to nail down the encoding.
- `from_env_errors_without_client_id()` — actionable message.
- `exchange_code_round_trips_form_post()` — wiremock `/oauth/token`
  asserts the form body and returns `{ access_token: "tok_xyz" }`.

### Integration test in `tests/oauth_flow.rs`

End-to-end test of `login_flow` with the browser step faked:

1. Mock `/oauth/authorize` and `/oauth/token` with wiremock.
2. Add an env var **`AHA_OAUTH_SKIP_BROWSER=1`** that, when set, makes
   `login_flow` print the authorize URL but skip `webbrowser::open`.
   The test sets this so it never spawns a real browser.
3. Spawn `login_flow` in a tokio task.
4. Poll the auth-server mock for the recorded `/oauth/authorize`
   request → extract `redirect_uri` and `state` from its query.
5. GET the redirect_uri with `?code=mock-code&state=<state>` to
   simulate the browser redirect back to the local listener.
6. Assert `login_flow` returns the mocked access token.

### CLI integration test in `tests/cmd_auth.rs`

Add a happy-path `aha auth login` (no `--with-token`) test that drives
the same shape and asserts the netrc is written. Reuse the env-var
override for the auth server + skip-browser flag.

## Telemetry / logging

- All informational lines go to stderr (matches the rest of the CLI).
- Print the authorize URL even when the browser opens — copy-pasteable
  fallback for headless sessions / WSL / containers.
- On the wait step, surface the redirect URI so the user knows where
  the callback is expected.

## Failure modes & messages

| Scenario | Message |
|---|---|
| `client_id` not configured | Step-by-step: how to register the OAuth app, env var name, fallback to `--with-token` |
| Browser open failed | Tell the user to paste the URL manually |
| Callback never arrived (timeout) | "OAuth callback didn't arrive in 5 minutes — was the consent prompt completed?" |
| `state` mismatch | "Aborting login: state parameter did not match (possible CSRF). Try again." |
| `?error=…` in callback | Echo Aha!'s reason verbatim |
| Token exchange non-2xx | Status + body, plus a hint to re-check the OAuth app config |
| `/api/v1/me` rejects the new token | Treat as a CLI bug — we just minted it; print a debug-mode hint |

## Open questions

1. **Loopback port range**: should we bind a specific high port to make
   firewall exceptions easier on locked-down machines? Probably not —
   the random port matches `gh` / `flyctl` and the consent screen only
   sees `127.0.0.1`, not the port until it follows the redirect.
2. **Scopes**: which scope string does Aha! accept for read access?
   Need to confirm during OAuth app registration; the API docs page is
   the source of truth.
3. **Token storage on the OAuth path**: we mark the netrc entry's
   `login` as `oauth` regardless of how the token was acquired. That's
   fine for read-back, but if/when we add refresh tokens we'll want to
   stash them alongside — possibly in a separate `~/.config/aha-cli/`
   file rather than netrc (since netrc is two fields only).
4. **Token revocation**: should `aha auth logout` also call Aha!'s
   `/oauth/revoke` (if it exists) so the token can't outlive the
   machine? Currently we just drop the netrc entry, which is fine for
   v0.1.

## Effort estimate

~half a day to land everything once the OAuth app is registered:
- Module + helpers: ~3 hours
- Unit + integration tests: ~1 hour
- Wire into CLI + docs: ~30 minutes
- Live smoke against `secure.aha.io`: ~30 minutes (fix surprises)
