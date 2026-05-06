# aha-tc — Implementation Plan

A Rust CLI for browsing and editing the Thoroughcare Aha! workspace from the
terminal. Drop-in successor to the unhelpful upstream `aha-cli` (which only
exists for extension development) and a thin sibling to `aha-mcp` (which targets
LLM clients, not humans).

## Goals

1. **Browse** products, releases, epics, features, requirements, todos, ideas
   from the terminal — table view by default, `--json` for piping.
2. **Edit** the entities the team actually touches day-to-day: create features,
   add comments, create/update todos.
3. **Single static binary**, no Node/Ruby runtime needed. Easy `brew tap` or
   `cargo install` distribution.
4. **Cooperate with existing auth**: read the same `~/.netrc` entry that
   `aha-cli` writes, fall back to `AHA_TOKEN` / `AHA_COMPANY` env vars (so it
   matches `aha-mcp`'s convention).

## Non-goals

- Replacing `aha-mcp` — the MCP server stays as the LLM-facing surface.
- Re-implementing the entire Aha! API. Scope = what humans browse, plus the
  small set of writes already supported by `aha-mcp`.
- A TUI. Plain stdout output, lean on the user's shell + `less` + `jq`.

## Reference points

- **`aha-mcp`** (`../aha-mcp/src/services/aha-service.ts`): the proven service
  layer. Re-implement its semantics in Rust:
  - 429 retry with `Retry-After` honoring (Aha caps ~5 req/sec).
  - Snowflake ID safety — IDs are 19-digit integers that exceed `i53`. Always
    deserialize as `String` (or `u64`) — never `f64`. The MCP server has a
    custom JSON pre-parser; in Rust this is handled by typing IDs as `String`
    in `serde` structs, which is cleaner.
  - Bounded fan-out concurrency (`mapWithConcurrency`) for the deep
    `get_feature` fetch — equivalent in Rust is `futures::stream::iter(...).buffer_unordered(N)`.
- **`@cedricziel/aha-js`** (the upstream OpenAPI-generated TS SDK that `aha-mcp`
  uses): can be referenced for endpoint shapes, but we do **not** need to bind
  to it. Aha! publishes an OpenAPI spec we can codegen from directly if
  desired (see "Optional: codegen" below).

## Tech stack

| Concern | Crate | Notes |
|---|---|---|
| CLI parsing | `clap` v4 with `derive` | de facto standard; nested subcommands |
| HTTP client | `reqwest` (rustls) | async; integrates with tokio |
| Async runtime | `tokio` | only need `rt-multi-thread`, `macros` features |
| JSON (de)serialization | `serde`, `serde_json` | derive on response structs |
| Retry/backoff | `reqwest-middleware` + custom 429 handler | small custom middleware — `reqwest-retry` doesn't honor `Retry-After` the way Aha! sends it |
| Output tables | `tabled` | derive-based; nice column control |
| Output colors | `owo-colors` | lightweight, no proc macros |
| Markdown rendering | `termimad` | for feature/comment bodies (Aha! returns HTML — pre-strip with `html2md`) |
| HTML→Markdown | `html2md` | descriptions and comments are HTML |
| `.netrc` parser | hand-rolled (~30 LOC) | the `netrc` crates are stale; Aha's entry format is non-standard anyway (`machine tcare type aha …`) |
| OAuth 2.0 client | `oauth2` | handles PKCE + token exchange; standard for Rust CLIs |
| OAuth callback server | `tiny_http` | small synchronous HTTP server for the localhost redirect URI |
| Browser launch | `webbrowser` | cross-platform `open`/`xdg-open` wrapper |
| Errors | `anyhow` (top-level) + `thiserror` (lib) | standard pattern |
| Logging | `tracing` + `tracing-subscriber` | `--verbose` flag bumps levels |
| Tests | `wiremock` | mock the Aha! HTTP API |

No optional async-std, no `tokio::main` macro magic past the entry point.

## Repo layout

```
aha-tc/
├── Cargo.toml          # workspace root; bin + lib in same crate
├── README.md
├── PLAN.md             # this file
├── .github/
│   └── workflows/
│       ├── ci.yml      # cargo test + clippy + fmt on PRs
│       └── release.yml # cross-compile macOS arm64/x86_64 + linux x86_64, attach to release
├── src/
│   ├── main.rs         # clap entry, dispatches to commands
│   ├── lib.rs          # re-exports for integration tests
│   ├── auth/
│   │   ├── mod.rs      # credential resolution (flags > env > netrc)
│   │   ├── netrc.rs    # tokenizer + read/upsert/remove
│   │   └── oauth.rs    # PKCE + local callback + token exchange
│   ├── client/
│   │   ├── mod.rs      # AhaClient: builds reqwest client, wraps every endpoint
│   │   ├── retry.rs    # 429 middleware
│   │   ├── pagination.rs  # async stream for paginated endpoints
│   │   └── models.rs   # Product, Feature, Release, Epic, Requirement, Todo, Comment, Idea
│   ├── output/
│   │   ├── mod.rs      # OutputFormat enum (Table | Json | Yaml)
│   │   ├── table.rs    # tabled-based renderers per entity
│   │   └── detail.rs   # single-entity vertical "kv" view (for `aha feature show`)
│   └── cmd/
│       ├── mod.rs
│       ├── products.rs    # aha products list
│       ├── features.rs    # aha features list/show/create/comment
│       ├── releases.rs    # aha releases list/show
│       ├── epics.rs       # aha epics list/show
│       ├── requirements.rs
│       ├── todos.rs       # aha todos list/show/create/update/done
│       ├── ideas.rs       # aha ideas list/show
│       ├── backlog.rs     # aha backlog [--release X] [--epic Y] — grouped feature view
│       └── auth.rs        # aha auth check / aha auth login (delegates to aha-cli; or write our own OAuth flow later)
└── tests/
    ├── netrc_parser.rs
    ├── retry.rs
    └── e2e_with_wiremock.rs
```

## Command surface

```
aha [global flags] <command> [subcommand] [flags]

Global flags:
  --subdomain <name>   Override AHA_COMPANY / netrc subdomain
  --token <token>      Override AHA_TOKEN / netrc token (rarely needed)
  --json               Emit JSON instead of tables
  --yaml               Emit YAML instead of tables
  -v, --verbose        Increase log level (-v info, -vv debug, -vvv trace)
  --no-color           Disable color output (also honors NO_COLOR env)

Commands:
  auth login [--subdomain <name>] [--with-token]
                                      Browser-based OAuth flow (default) or
                                      paste a personal API key (--with-token,
                                      reads from stdin so the token never
                                      lands in shell history).
  auth check                          Verify stored credentials are valid
  auth whoami                         Print authenticated user
  auth logout                         Remove credentials from .netrc

  products list

  releases list [--product TC] [--parking-lot] [--shipped]
  releases show <REF>                 e.g. TC-R-15

  epics list [--product TC] [--release TC-R-15]
  epics show <REF>                    e.g. TC-E-42

  features list [--product TC] [--release TC-R-15] [--epic TC-E-42]
                [--assignee <email>] [--tag <tag>] [--status <status>]
                [--updated-since <date>] [-q <query>]
  features show <REF>                 Deep view: feature + requirements + comments + todos
  features create --product TC --name "..." [--description ...] [--tag ...]
                  [--assignee <email>]
  features comment <REF> --body "..."

  requirements show <REF>
  requirements comment <REF> --body "..."

  todos list [--mine] [--feature TC-1109] [--status pending|completed]
  todos show <ID>
  todos create --feature TC-1109 --name "..." --body "..." [--due 2026-05-20] [--assignee <email>]
  todos update <ID> [--status completed] [--name ...] [--body ...] [--due ...]
  todos done <ID>                     Sugar for `todos update <ID> --status completed`

  ideas list [--product TC] [--status <status>]
  ideas show <REF>

  backlog [--product TC] [--release X] [--epic Y]
                                     Grouped feature view: status → epic → feature
                                     This is the "browse the roadmap" view we
                                     don't get from any existing tool.
```

## Auth

### Credential resolution (every command except `auth login`)

Priority order — first hit wins:

1. CLI flags (`--token`, `--subdomain`)
2. Env: `AHA_TOKEN`, `AHA_COMPANY` (matches `aha-mcp` convention)
3. `~/.netrc` entry, in two flavors:
   - **`aha-tc`-native format** (what we write): standard
     `machine <subdomain>.aha.io login oauth password <token>`. Any tool that
     speaks netrc will see a usable Bearer-shaped credential.
   - **Upstream `aha-cli` format** (interop): the npm CLI writes
     `machine <subdomain> type aha email <email> token <token> url https://<subdomain>.aha.io:443`.
     Non-standard `type` / `email` / `token` / `url` fields trip every netrc
     parser crate I've looked at. We hand-write a ~30-LOC tokenizer that
     accepts both shapes.

Failure to resolve credentials prints a one-liner pointing at `aha auth login`.

### `aha auth login` — OAuth 2.0 with PKCE + local callback

Mirrors the flow upstream `aha-cli` uses, and `gh auth login --web`,
`flyctl auth login`, etc. Concretely:

1. CLI prompts for the Aha! subdomain (or reads `--subdomain` / `AHA_COMPANY`).
2. CLI generates a PKCE `code_verifier` + `code_challenge` (S256).
3. CLI starts a local HTTP listener on a random high port (e.g. via
   `std::net::TcpListener::bind("127.0.0.1:0")` and reads back the assigned
   port).
4. CLI opens the browser to:
   ```
   https://secure.aha.io/oauth/authorize?
     client_id=<aha-tc client id>&
     redirect_uri=http://127.0.0.1:<port>/callback&
     response_type=code&
     code_challenge=<challenge>&
     code_challenge_method=S256&
     scope=&
     state=<random>
   ```
5. User authorizes in browser; Aha! redirects to the local callback with
   `?code=…&state=…`.
6. Local server validates `state`, captures `code`, returns a tiny "you can
   close this tab" HTML page, then shuts down.
7. CLI POSTs to `https://secure.aha.io/oauth/token` with
   `grant_type=authorization_code`, `code`, `code_verifier`, `client_id`,
   `redirect_uri`. Receives `access_token` (and possibly `refresh_token`).
8. CLI writes `~/.netrc` (creating with `0600`, merging if it exists — never
   clobber other entries).

**Crates:** `oauth2` (handles PKCE + token exchange), `tiny_http` (callback
server), `webbrowser` (cross-platform browser launch). Total ~150 LOC of glue.

**OAuth client registration — open question for the team.** Aha! requires a
registered OAuth app (`client_id`). Three options:

- (a) **Reuse `aha-cli`'s `client_id`.** The upstream package is open source on
  npm; the ID is plain text. Functional but feels off — we'd be shipping
  someone else's app identifier.
- (b) **Register a `aha-tc` OAuth app under the ThoroughCare Aha! account.**
  Cleanest. Bake the `client_id` into the binary, no `client_secret` because
  PKCE makes the public-client model safe. ~5 minutes of setup in the Aha!
  admin UI (`/settings/account/integrations`).
- (c) **Skip OAuth, only support `--with-token`.** User generates a personal
  API key at `/settings/personal/developer` and pipes it into
  `aha auth login --with-token`. Simplest to implement (~20 LOC) but worse UX.

**Recommendation: (b), with (c) as a `--with-token` fallback for headless /
CI environments.**

### `aha auth login --with-token`

Reads a token from stdin (so it doesn't appear in `ps` or shell history),
verifies it with a `GET /api/v1/me`, writes `.netrc`. ~20 LOC.

### `.netrc` writes

- Create with mode `0600` if it doesn't exist.
- If it exists, parse → replace any matching `machine <subdomain>.aha.io`
  block → rewrite atomically (`tempfile` + `rename`).
- Don't touch other tools' entries.
- Document in `--help` exactly what we write so users can audit.

## Implementation phases

### Phase 0 — scaffolding (~1 hour)
- `cargo new --bin aha-tc` (already created the dir; just add `Cargo.toml`).
- `clap` skeleton with `aha auth check` as the only working command.
- CI workflow (`fmt + clippy + test`).

### Phase 0.5 — auth (~half day)
- Hand-rolled netrc tokenizer (handles both upstream-`aha-cli` and standard
  formats), with `read` / `upsert` / `remove` operations.
- `auth login --with-token` first — useful immediately and validates the
  netrc round-trip.
- `auth check` (`GET /api/v1/me`).
- `auth login` (browser OAuth + PKCE) — gated on team picking option (a/b/c)
  above. If (b), this is ~150 LOC of glue around `oauth2` + `tiny_http` +
  `webbrowser` once we have the `client_id`.
- `auth logout` — netrc rewrite minus our entry.

### Phase 1 — read-only browse (~half day)
- `AhaClient` with bearer auth + base URL building.
- 429 retry middleware.
- Models for `Product`, `Release`, `Epic`, `Feature`, `Requirement`, `Todo`,
  `Comment`, `Idea` — IDs as `String`, dates as `chrono::DateTime<Utc>`.
- Pagination helper (`async-stream` yielding pages).
- Table output for `products list`, `releases list`, `epics list`, `features list`.
- `features show` deep fetch — port the bounded-concurrency fan-out from `aha-mcp`.

### Phase 2 — write paths (~half day)
- `features create`
- `features comment` / `requirements comment`
- `todos create` / `todos update` / `todos done`

### Phase 3 — the killer feature: `backlog` (~half day)
- Group features by `release → epic → status`.
- Format: collapsible-ish (release header, epic sub-header, feature row).
- Honor `--release` / `--epic` filters.
- This is the view that justifies the tool existing at all.

### Phase 4 — polish (~half day)
- `--json` / `--yaml` output paths through every command.
- Markdown rendering for `features show` description (HTML → MD → terminal).
- Shell completions (`clap_complete`).
- `brew tap thoroughcare/tap` formula + GitHub release workflow that
  cross-compiles macOS arm64/x86_64 + linux x86_64.

**Total estimate: ~2.5 dev days for a polished v0.1** (3 days if we go with
OAuth flow option (b) and need to register the Aha! app + iterate on the
flow).

## Optional: codegen path

Aha! ships an OpenAPI spec at `https://www.aha.io/api/swagger.json`. We could
generate the `models.rs` + low-level client with `progenitor`. Pros: less
hand-typed boilerplate, schema drift is a `cargo build` away. Cons: the
generated code is large and ugly, and we still need a hand-written facade for
the parts that need pagination / retry / fan-out. **Recommendation: skip
codegen for v0.1, hand-write the ~10 endpoints we use.** Revisit if scope
expands.

## Open questions for the team

1. **Repo location** — keep this as a standalone repo under
   `github.com/thoroughcare/aha-tc`, or fold it into `aha-mcp` as a sibling
   crate (Rust workspace) and share nothing? They share zero code (different
   languages), so probably standalone.
2. **Distribution** — `brew tap` only, or also publish to `crates.io`?
3. **Output defaults** — table-by-default is friendly for humans, but is
   anyone going to pipe heavily to `jq`? If yes, default to JSON when stdout
   is not a TTY (a la `gh`).
4. **Naming** — `aha-tc` clashes with nothing but is uninspired. Options:
   `tcaha`, `tcare-aha`, just `aha` (collides with the ANSI→HTML brew tool).

## Risks

- **Snowflake IDs** — easy to lose precision if any struct types an `id` as
  `i64`. Add a clippy lint or a test that round-trips a known 19-digit ID.
- **Rate limits** — 5 req/sec is tight for the deep `features show`. Keep the
  bounded-concurrency cap from `aha-mcp` (3 in-flight).
- **OpenAPI drift** — if Aha! rotates fields, hand-written models break
  silently on serialization. `serde(default)` + `#[serde(other)]` catch-alls
  on enums minimize surprise.
- **Auth coupling to upstream `aha-cli`** — if we read `.netrc` written by it,
  changes in their format would break us. Document the format we expect, fall
  back to env vars cleanly.
