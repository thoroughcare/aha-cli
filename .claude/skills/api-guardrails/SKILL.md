---
name: api-guardrails
description: |
  Enforce the Aha! API client contract when adding or modifying anything that
  talks to `*.aha.io`: rate-limit headroom, retry reuse, bearer-token
  hygiene, pagination, and the wiremock test pattern.
  TRIGGER when: editing `src/client/**`, adding a new endpoint, adding a new
  command that fans out HTTP calls, touching authentication/credential code,
  or writing a new `examples/probe_*.rs`.
  SKIP when: the change is purely presentational (`src/cmd/` row projections,
  `src/output/`), docs-only, or test-fixture-only.
---

# Aha! API Guardrails

The Aha! API has a ~5 req/sec soft cap and returns `429` with a
`Retry-After` header when exceeded. The client encodes this contract in a
small number of places. New code must reuse them rather than re-derive
them.

## Phase 1: Route every call through `AhaClient`

- Every HTTP request **must** go through an `AhaClient` method. Do not
  build a fresh `reqwest::Client` in a command, an example, or a test
  helper. That path skips the bearer header (`set_sensitive(true)`), the
  user-agent, and the retry loop.
- Library-level GETs go through `get_json` (typed) or `get_json_raw`
  (probes / examples that want raw `serde_json::Value`).
- Anything more exotic (POST, streaming download, non-JSON response) goes
  through `send_with_retry` so the 429 loop still applies.

The chokepoints:
- `src/client/mod.rs` — `AhaClient::new` / `with_base_url`, header setup.
- `src/client/retry.rs` — `send_with_retry`, `get_json`, retry/backoff.
- `src/client/resources.rs` — per-resource list/show methods.

## Phase 2: Respect the rate budget

- Aha! caps the API at ~5 req/sec. Any parallel fan-out caps concurrency
  at `FANOUT_CONCURRENCY = 3` (see `src/client/resources.rs`). Reuse the
  constant — do not introduce a new one. If a new flow needs different
  headroom, propose the change explicitly; don't smuggle it in.
- Use `futures::stream::iter(...).buffer_unordered(FANOUT_CONCURRENCY)`
  for parallel fan-out (the established pattern, e.g. `feature_show`).
  Don't spawn unbounded `tokio::spawn` tasks against the Aha! host.
- Sequential pagination uses `pagination.total_pages` — don't try to
  parallelize page walks. Aha! has flagged accounts for that before.

## Phase 3: Retry and error mapping

- 429 retry lives in `send_with_retry`. Do **not** add a second retry
  layer on top of it (e.g. a `for attempt in ...` loop around a
  `get_json` call). One retry layer, in one place.
- `Retry-After` is honored when present; fallback is exponential backoff
  capped at `MAX_RETRIES = 3`. If you need different policy for a new
  call type, change the central function — don't fork it.
- Non-2xx responses from `get_json` bubble up as `anyhow` errors that
  include the status and the body snippet. Preserve that shape; CLI
  error messages depend on it (`401`, `403`, etc. are matched on by the
  `auth` skill and the user-facing prompts).

## Phase 4: Credentials and secret hygiene

- `Credentials.token` is sensitive. The authorization header is built
  with `HeaderValue::set_sensitive(true)` so `reqwest` redacts it from
  debug logs — preserve that when touching `with_base_url`.
- **Never** log, print, snapshot, or include in an error message:
  - the raw token,
  - the full `Authorization` header,
  - the contents of `~/.netrc`,
  - the contents of `.env`.
- When writing a wiremock test that asserts the header, match on
  `Bearer testtoken` with a synthetic token (see the existing test in
  `src/client/mod.rs`). Do not paste a real token into a fixture, even
  a "test" account's.
- `AHA_BASE_URL` is the undocumented escape hatch that points the
  client at a mock server. It is intentionally not surfaced in `--help`.
  Keep it that way — that env var is for tests, not users.

## Phase 5: Tests for new endpoints

Every new client method needs at least:
1. **Happy path** — wiremock returns 200 with a representative body;
   assert the typed model decodes the fields you care about.
2. **Non-2xx** — wiremock returns 401 or 404; assert the `anyhow` error
   includes the status code in its message.
3. **(If the flow fans out)** verify concurrency stays bounded —
   typically by structure (use `FANOUT_CONCURRENCY`) rather than a
   timing test.

CLI-level smoke tests go in `tests/cmd_*.rs` and use `assert_cmd` +
`AHA_BASE_URL` pointed at a `MockServer`. Mirror the existing files —
one per command surface.

## Phase 6: Updating user-visible surface

If you added or removed a command, or changed a JSON shape that's
documented in `README.md`:
- Update the command table in `README.md`.
- Update the JSON shape example if `features show` / `todos show`
  output changed.
- Add a recipe to `docs/recipes.md` if the new command does something a
  scripted user would reach for.

## Reminders
- One retry layer. One concurrency cap constant. One auth header builder.
- No fresh `reqwest::Client` outside `AhaClient`.
- Tokens never leave the process address space via logs, prints,
  snapshots, or error messages.
- Probes and examples (`examples/probe_*.rs`) use `get_json_raw` —
  they are for diagnosis, not for shipping features. If a probe becomes
  permanent, promote it to a typed model + resource method.
