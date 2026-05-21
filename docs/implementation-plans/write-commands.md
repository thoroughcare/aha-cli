# Write / edit commands — implementation plan

The v0.1 surface (see [`v0.1-read-only.md`](v0.1-read-only.md))
shipped read-only by design. This plan covers the write path —
creating and updating features, requirements, todos, and comments —
mirroring the aha-mcp service layer
(`../aha-mcp/src/services/aha-service.ts`) so the two tools stay
interchangeable.

The bulk of the design here is now **implemented** (commits
`1fc9c3e` + `971aaa9`); the **Status** section at the end is the
source of truth for what's shipped vs. still ahead. Use the design
sections to understand *why* the surface looks the way it does and
to keep it that way as it evolves.

## Scope

A small, focused first cut. Match aha-mcp's write surface so the two
tools stay interchangeable. Anything that needs a UI-level
interaction (release planning, custom field editing, bulk moves) is
out of scope.

| Command                                          | Verb / endpoint                                     |
| ------------------------------------------------ | --------------------------------------------------- |
| `aha features create --product <P> --name <N>`   | `POST /products/<P>/features`                       |
| `aha features edit <REF> [field flags]`          | `PUT  /features/<REF>`                              |
| `aha features comment <REF>`                     | `POST /features/<REF>/comments`                     |
| `aha requirements create --feature <REF> --name <N>` | `POST /features/<REF>/requirements`             |
| `aha requirements edit <REF> [field flags]`      | `PUT  /requirements/<REF>`                          |
| `aha requirements comment <REF>`                 | `POST /requirements/<REF>/comments`                 |
| `aha todos create --on <ref> --name <N>`         | `POST /tasks`                                       |
| `aha todos edit <ID> [field flags]`              | `PUT  /tasks/<ID>`                                  |
| `aha todos done <ID>` / `aha todos reopen <ID>`  | `PUT  /tasks/<ID>` (status convenience)             |

Explicitly out of scope for the first cut:

- Deletes (`DELETE /features/...`, `DELETE /tasks/...`,
  `DELETE /requirements/...`). The API supports them but the blast
  radius is high; defer until there's demand and a confirmation UX
  worked out.
- Releases / epics / ideas writes. Not in MCP, no clear demand.
- Attachments upload. Separate body of work (multipart), not blocked
  on this plan.

## Surface design

### Reading text bodies — the multi-line problem

Feature descriptions and comment bodies are long-form HTML / Markdown.
We need three patterns and one rule:

- **`--body <text>`** — short body inline. Tolerates `\n` but expect
  one-line use.
- **`--body-file <path>`** — read the body from a file. `-` means
  stdin (binary-safe, reads until EOF). Mirrors `gh issue create
  --body-file`.
- **`--editor`** — spawn `$EDITOR` on a tempfile, use the saved
  contents as the body. Defaults: `$VISUAL`, then `$EDITOR`, then
  `vi`. Suffix the tempfile `.md` so editors get syntax hints. If the
  resulting body is empty after trimming, abort with "no body
  provided — aborting (mirrors `git commit`'s empty-message rule)".
- **Default when none is given on an *edit*-style command** — open
  `$EDITOR` pre-populated with the existing body so users don't
  accidentally clobber a long description.

Rule: exactly one of `--body` / `--body-file` / `--editor` per
invocation. `clap`'s `conflicts_with_all` handles that statically.

### Confirmations and dry-run

Mutations only run after one of:

- `--yes` / `-y`: skip the prompt entirely.
- TTY prompt (`"create feature 'X' in product TC? [y/N]"`) when
  stdout *or* stdin is a TTY. Pipes (e.g. agent shells) implicitly
  require `--yes` — bail with a clear error if neither holds.
- `--dry-run`: print the request that *would* go out (method, path,
  pretty-printed JSON body) and exit 0 without touching the wire.

This is the same shape as `gh pr create` minus the editor flow that's
covered separately above. The implementation lives in one helper
(`cmd::write::confirm`) so every write command picks it up.

### Output

Writes return the just-created/updated entity. Print the standard
`show`-style detail (kv view for TTY, full JSON when piped). On
create, also write the new entity's `id` / `reference_num` to stderr
so scripts can capture it without parsing JSON — e.g.:

```text
Created feature TC-1234 (id=782…6886)
```

Stderr keeps the success line out of the JSON pipe.

## Module layout

```
src/
├── client/
│   ├── mod.rs            # AhaClient — add post_json / put_json helpers
│   └── resources.rs      # add: create_feature, update_feature,
│                         #      create_feature_comment, …
├── cmd/
│   ├── mod.rs
│   ├── features.rs       # add `create`, `edit`, `comment`
│   ├── requirements.rs   # add `comment`
│   ├── todos.rs          # add `create`, `edit`, `done`, `reopen`
│   └── write.rs          # NEW — body-reading + confirm + dry-run helpers
└── cli.rs                # extend FeaturesCommand / TodosCommand /
                          # RequirementsCommand variants
```

No new top-level modules. Keep the write path threaded through the
existing per-resource files so the related read and write surfaces
sit side by side (e.g. `aha features show` and `aha features edit`
are in the same file).

## Client layer

`AhaClient` currently only exposes `get_json`. Add two siblings, both
returning the response body deserialised as `T`:

```rust
impl AhaClient {
    pub(crate) async fn post_json<B, T>(&self, path: &str, body: &B) -> Result<T>
    where
        B: serde::Serialize,
        T: serde::de::DeserializeOwned;

    pub(crate) async fn put_json<B, T>(&self, path: &str, body: &B) -> Result<T>
    where
        B: serde::Serialize,
        T: serde::de::DeserializeOwned;
}
```

Both reuse the retry middleware, the same `Bearer` auth header, and
the same envelope-aware error mapping as `get_json` (status + body
captured into `anyhow::Error`). They share one private
`send_json_request(method, path, body)` to avoid copy-paste.

### Endpoint methods (mirror aha-mcp shapes)

Snake-case keys match Aha!'s wire format. Aha! wraps create/update
bodies in a top-level resource key (`feature`, `task`, `comment`,
…) — model that with small request structs rather than `serde_json::
json!()` so we get compile-time field checking and the same forward
compatibility (`#[serde(skip_serializing_if = "Option::is_none")]`).

```rust
// src/client/resources.rs
#[derive(Default, Serialize)]
pub struct FeatureCreate<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<&'a str>,                // comma-separated, Aha's convention
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to_user: Option<&'a str>,    // email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_status: Option<&'a str>,
}

#[derive(Default, Serialize)]
pub struct FeatureUpdate<'a> { /* same shape, all optional */ }

#[derive(Default, Serialize)]
pub struct RequirementUpdate<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,         // plain string on write; Aha! returns it as { body } on read
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_status: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to_user: Option<&'a str>,
}

#[derive(Default, Serialize)]
pub struct TodoCreate<'a> {
    pub name: &'a str,
    pub body: &'a str,
    pub taskable_type: TaskableType,          // enum: Feature | Requirement | Release | Epic
    pub taskable_id: &'a str,                 // reference_num or numeric id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<&'a str>,            // ISO 8601 date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to_users: Option<&'a [String]>,
}
```

Method signatures:

```rust
pub async fn create_feature(&self, product: &str, body: &FeatureCreate<'_>) -> Result<Feature>;
pub async fn update_feature(&self, id_or_ref: &str, body: &FeatureUpdate<'_>) -> Result<Feature>;
pub async fn create_feature_comment(&self, id_or_ref: &str, body: &str) -> Result<Comment>;
pub async fn update_requirement(&self, id_or_ref: &str, body: &RequirementUpdate<'_>) -> Result<Requirement>;
pub async fn create_requirement_comment(&self, id_or_ref: &str, body: &str) -> Result<Comment>;
pub async fn create_todo(&self, body: &TodoCreate<'_>) -> Result<Todo>;
pub async fn update_todo(&self, id: &str, body: &TodoUpdate<'_>) -> Result<Todo>;
```

Each unwraps the `{ "feature": {...} }` / `{ "task": {...} }`
envelope via the existing `OneEnvelope<T>` (it already covers
`feature`, `task`, `requirement`, …).

### Error mapping

422 from Aha! includes the validation messages — surface them
verbatim, don't swallow into a generic "bad request". Pattern:

```rust
if status == 422 {
    anyhow::bail!("Aha! rejected the request (422): {body}");
}
if status == 404 {
    anyhow::bail!("not found: {path} — check the reference or id");
}
```

Everything else falls through to the existing `get_json` error shape
(`HTTP <status>: <body>`).

## CLI surface

```text
aha features create
    --product <PRODUCT>
    --name <NAME>
    [--description <TEXT> | --description-file <PATH> | --editor]
    [--tags <TAG,TAG>]
    [--assignee <EMAIL>]
    [--status <STATUS>]
    [--dry-run] [-y|--yes]

aha features edit <REF>
    [--name <NAME>]
    [--description <TEXT> | --description-file <PATH> | --editor]
    [--tags <TAG,TAG>]              # replaces; use `--add-tag` / `--remove-tag`
    [--add-tag <TAG>]... [--remove-tag <TAG>]...
    [--assignee <EMAIL>]
    [--status <STATUS>]
    [--dry-run] [-y|--yes]

aha features comment <REF>
    [--body <TEXT> | --body-file <PATH> | --editor]
    [--dry-run] [-y|--yes]

aha requirements edit <REF>
    [--name <NAME>]
    [--description <TEXT> | --description-file <PATH> | --editor]
    [--status <STATUS>]
    [--assignee <EMAIL>]
    [--dry-run] [-y|--yes]

aha requirements comment <REF>
    [--body <TEXT> | --body-file <PATH> | --editor]
    [--dry-run] [-y|--yes]

aha todos create
    --on <REF>                     # feature ref, requirement ref, release, or epic
    [--on-type feature|requirement|release|epic]   # inferred from --on prefix when omitted
    --name <NAME>
    [--body <TEXT> | --body-file <PATH> | --editor]
    [--due <YYYY-MM-DD>]
    [--assignee <EMAIL>]...
    [--dry-run] [-y|--yes]

aha todos edit <ID>
    [--name <NAME>] [--body … | --body-file … | --editor]
    [--status pending|completed]
    [--due <YYYY-MM-DD>]
    [--assignee <EMAIL>]...
    [--dry-run] [-y|--yes]

aha todos done <ID>        # convenience: PUT { status: "completed" }
aha todos reopen <ID>      # convenience: PUT { status: "pending" }
```

### Tag merge semantics on `features edit`

Aha! treats `tags` as a full replacement — sending `tags: "a,b"`
wipes any existing tag not in that list. Three patterns:

1. `--tags a,b` — replace (matches the API verbatim).
2. `--add-tag x` / `--remove-tag y` — fetch the existing feature,
   compute the new set client-side, send a full replace. One extra
   GET; worth it because tag edits are almost always additive.
3. Mixing `--tags` with `--add-tag` / `--remove-tag` is an error
   (clap `conflicts_with`).

### Inferring `--on-type` from `--on`

Reference prefixes encode the parent type: `TC-1234` is a feature,
`TC-R-12` a release, `TC-E-42` an epic, `TC-1234-5` a requirement.
Implement a small parser; surface a clear error if the user supplied
something ambiguous (a numeric id without `--on-type`).

## Status

The bulk of this plan shipped in commits `1fc9c3e` ("Add write
commands: features/requirements/todos create, edit, comment") and
`971aaa9` ("Fix write commands found broken in live smoke test"),
and was lightly retro-documented after the fact. Use this section
as the source of truth for what's in vs. still ahead — the design
sections above describe the shape on disk today unless flagged
otherwise.

### Shipped

- Client plumbing: `post_json` / `put_json` on `AhaClient`, request
  structs (`FeatureCreate`, `FeatureUpdate`, `RequirementCreate`,
  `RequirementUpdate`, `TodoCreate`, `TodoUpdate`), and 422 / 404
  error mapping.
- `src/cmd/write.rs` — shared body resolution (`--body` /
  `--body-file` / `--editor`), TTY confirm, `--dry-run`.
- Retry middleware already guards `POST` against accidental
  re-issue (`src/client/retry.rs:77,132`) — risk in the original
  plan is closed.
- Commands:
  - `aha features create / edit / comment` (with `--add-tag` /
    `--remove-tag` tag-merge).
  - `aha requirements create / edit / comment`.
  - `aha todos create / edit / done / reopen`.
- Tests: `tests/cmd_write.rs` plus write cases folded into
  `tests/cmd_features.rs` and `tests/cmd_todos.rs`.
- Docs: README "Write commands" section, recipes in
  `docs/recipes.md`.

### Still ahead

These are open or deferred — none are blocking but each is a real
follow-up:

- **Bulk `aha todos done <ID>...`** — open question in the original
  plan; defer until the first user request.
- **Workflow-status name vs id** — `--status "In progress"` is
  documented to work via case-insensitive name match against the
  feature's workflow; worth a live smoke test to confirm Aha!
  accepts the name on the wire and to add a regression test.
- **Empty-edit guard** — confirm `aha {features,requirements,todos}
  edit <REF>` with no field flags bails clearly (no meaningless
  PUT). Spot-check; add a test if missing.
- **Deletes** (`features` / `requirements` / `tasks`) — deferred;
  needs a confirmation UX before exposing.
- **Attachments upload** — multipart, separate work, not blocked
  on this plan.

## Testing

Same shape as the read tests — wiremock fixtures + `assert_cmd`.

### Unit tests (in `client/resources.rs`)

- `create_feature_serializes_envelope` — assert the wire body is
  `{"feature": {"name": "...", ...}}` with `description: null` *not*
  present (skip-if-none works).
- `update_todo_serializes_only_changed_fields` — same.
- `taskable_type_round_trips` — `TaskableType::Feature` ↔ `"Feature"`.
- `tag_merge_computes_union_and_difference` — pure function,
  no HTTP.

### HTTP integration tests (`tests/cmd_*.rs`)

Add to existing files when the resource matches; one new
`tests/cmd_write.rs` for the cross-cutting `--dry-run` and `--yes`
behaviour.

- `cmd_features.rs::create_feature_posts_envelope` — wiremock asserts
  POST body shape, returns the canned feature, command exits 0,
  stderr contains "Created feature TC-…".
- `cmd_features.rs::edit_feature_dry_run_makes_no_request` —
  wiremock asserts zero requests received.
- `cmd_features.rs::edit_feature_tag_merge_does_one_get_one_put` —
  wiremock counts both calls.
- `cmd_todos.rs::done_sets_status_completed` — PUT body matches
  `{"task": {"status": "complete"}}`. (Aha! rejects `"completed"` —
  empirically confirmed via `examples/probe_task_status.rs`.)
- `cmd_write.rs::no_tty_no_yes_bails` — verify the safety rail.
- `cmd_write.rs::editor_path_uses_env_editor` — fake `$EDITOR` with a
  shell script that writes a known string into the tempfile.

### What we deliberately don't test

- Live API. As today: never call out in CI. Live smoke is documented
  in `CONTRIBUTING.md` so a release manager can run a paste-token
  test against a junk feature in the tcare workspace before tagging.

## Safety considerations

- **No accidental tag wipes.** The `--add-tag` / `--remove-tag`
  pattern exists specifically because the API verb is "replace",
  which has bitten users in other tools.
- **Editor-empty aborts.** Empty body after trim → "aborting, empty
  body". Same convention as `git commit`. Avoids accidental
  "comment" containing only a stray newline.
- **Pipes default to no-op.** No `--yes` on a non-TTY = exit 1 with a
  pointer to `--yes`. An LLM-driven agent calling `aha features edit`
  has to opt in to mutation explicitly, every time.
- **No `--force`-style override of 422 validation.** If Aha! rejects
  the payload, surface the message and let the user fix it; we
  don't have a use case for ignoring server-side validation.

## Open questions

1. **Should `aha todos done` accept a list of ids?** Convenient for
   sweeping a feature clean. Probably yes — clap `Vec<String>` with
   per-id confirmations (or one bulk confirm under `--yes`). Defer
   to the first user request.
2. **Workflow status names vs ids.** `--status "In progress"` vs
   `--status s_8f3…`. The MCP server takes the name. Match that;
   match-case-insensitively so the human-friendly form works.
3. **Editor flow on non-interactive shells.** `--editor` on a pipe
   should be an error, not a silent fallback to "empty body". The
   confirm helper already bails on no-TTY; reuse the check.
4. **Idempotency.** Aha! has no idempotency-key support documented.
   If a write times out post-send, the user may end up with a
   duplicate. We don't paper over this; document it. The existing
   429 retry middleware is *not* applied to writes — only GET (we
   should add an explicit guard so a future change doesn't quietly
   start retrying mutating verbs).

## Known-broken APIs

- **`PUT /tasks/<id>` silently no-ops `task.status`.** Confirmed live
  on 2026-05-21 against `tcare.aha.io`: the server returns 200 OK and
  accepts the value `"complete"` / `"pending"` (rejects `"completed"`
  with 400), but a follow-up GET shows `status` unchanged. Sub-resource
  candidates (`POST /tasks/:id/complete`, `…/done`) return 404. `PATCH`
  with the same body has the same 200-but-no-effect behavior. The
  `aha-mcp` service models the same broken endpoint and presumably has
  the same bug.

  Mitigation: `todos done` / `reopen` / `todos edit --status` re-GET
  the task after the PUT and surface a clear error when the server
  silently ignored the write. The commands stay in the surface because
  Aha! may fix the API; if/when they do, the verify step naturally
  starts succeeding without code changes.

  Probe: `cargo run --example probe_task_status -- <task-id>` against
  any live task.

## Risks

- **Retry middleware on writes.** Today `retry.rs` retries 429 on any
  request. For writes, retrying after a partial 429 could
  double-create. Mitigation: limit retries to idempotent methods
  (`GET`, `HEAD`, `PUT` — Aha!'s `PUT` is replace-style and so is
  idempotent in practice). Explicitly skip retry on `POST`. Add a
  test that asserts a 429 on POST surfaces the error rather than
  retrying.
- **Snowflake IDs in request bodies.** Same concern as responses:
  IDs must stay strings. Request structs already use `&str`, so this
  is handled, but add a regression test that sends a 19-digit id and
  asserts the wire payload preserves it verbatim.
- **Tag list parsing.** Aha! accepts tags as a comma-separated string
  in the wire format. `--tags "a, b"` (with a space) should still
  work; trim each entry client-side before sending.
