---
name: review-aha-ticket
description: |
  Fetch and review a ThoroughCare Aha! feature or requirement using the `aha` CLI.
  Pulls description, acceptance criteria, status, requirements, todos, and comments,
  then summarises the ticket for the user. Read-only — does not switch branches or
  modify code.
  TRIGGER when: user asks to review a tcare Aha ticket, "look at TC-1234", "what does
  TC-1234 say", pastes a `tcare.aha.io/develop/features/...` URL, or types
  /review-aha-ticket [TICKET-ID].
  SKIP when: user is asking to *start* implementation (use start-ticket), respond to
  QE feedback (use failed-qe), or create a new ticket (use create-ticket).
---

# Review Aha Ticket

Fetch and review a ThoroughCare Aha! ticket. Argument: ticket reference (e.g., `TC-1713`,
`TC-1713-1` for a requirement, or a full `https://tcare.aha.io/develop/features/...` URL).

$ARGUMENTS

If no argument is given, ask for one before proceeding.

---

## Phase 1: Identify the Ticket

- Feature IDs look like `TC-1234`.
- Requirement IDs look like `TC-1234-1` (parent feature plus suffix).
- If the user pastes a URL, extract the ID from the last path segment.

---

## Phase 2: Ensure the `aha` CLI is Authenticated

The CLI binary is `aha` (Rust binary at `~/.cargo/bin/aha`). It resolves credentials in
this order — **first hit wins**:

1. `--token` / `--subdomain` flags
2. `AHA_TOKEN` / `AHA_COMPANY` env vars
3. `~/.netrc` entry written by `aha auth login` (the persistent default)

**Default behaviour: invoke `aha` bare and let the CLI resolve from `~/.netrc`.**
No env sourcing, no `AHA_COMPANY=tcare` prefix — the netrc entry carries both the
subdomain (encoded in the host) and the token. Only fall back to `.env`/env vars
if the netrc has no entry and writing one isn't appropriate (e.g. transient CI).

### If the CLI errors out

If `aha auth check` (or a real command) says "no Aha! credentials found" or returns
`401 Unauthorized`, do **not** silently retry. Surface the error and pick one path:

- **Persist credentials** (recommended — works for every future `aha` invocation,
  no env setup): generate a personal API token at
  `https://tcare.aha.io/settings/api_keys` → **Create API key**, then:
  ```
  printf '%s' "<personal-api-token>" | aha auth login --with-token --subdomain tcare
  ```
  This writes `machine tcare.aha.io login oauth password <token>` to `~/.netrc`
  with mode `0600`. Verify with `aha auth check`.

- **One-off for the current shell** (handy when the user explicitly doesn't want
  the token on disk):
  ```
  export AHA_COMPANY=tcare
  export AHA_TOKEN=<personal-api-token>
  ```

- **Legacy `.env` fallback** — only if the user's workflow depends on
  `/Users/oliver/Repos/tc/.env`:
  ```
  set -a; source /Users/oliver/Repos/tc/.env; set +a; \
    AHA_COMPANY=tcare aha auth check
  ```
  Prefer migrating the token into netrc once via `aha auth login --with-token`.

---

## Phase 3: Fetch Ticket Details

Always pass `--no-color` so output is clean. Stream the raw CLI output back as part of
your response so the user can see the source.

### 3a. Feature (e.g., `TC-1713`)
```
aha features show <ID> --no-color
```
This output includes requirements, comments, and to-dos.

### 3b. Requirement (e.g., `TC-1713-1`)
```
aha requirements show <ID> --no-color
```
Also fetch the parent feature (`TC-1713`) — context like description and acceptance
criteria often lives on the parent.

### 3c. If JSON is easier to parse
Add `--json` to either command. Useful when you need to extract specific fields
programmatically.

### 3d. If the netrc isn't set up yet
The bare invocations above assume `~/.netrc` already has the credentials (the
default after `aha auth login`). If they error with "no Aha! credentials found"
or `401`, surface the error and prompt the user to run
`aha auth login --with-token --subdomain tcare` (or export `AHA_TOKEN` /
`AHA_COMPANY` for a one-shot). Then retry — do NOT pre-source `.env`
defensively.

---

## Phase 4: Summarise for Review

Present back to the user, in this order:

1. **Title & status** — current workflow state (e.g., Ready to develop, In progress, QE).
2. **Description / user story** — paraphrase the intent in 2–3 sentences.
3. **Acceptance criteria** — list verbatim if short; summarise if long.
4. **Requirements** — for a feature, list each requirement with its status.
5. **Open todos** — surface anything outstanding.
6. **Comments** — surface PM/QE/dev comments that change scope or flag blockers.
   Include the author and date so the user can follow up.
7. **Open questions / risks** — based on the above, what is ambiguous, missing, or
   risky from an implementation standpoint?

End with a short pointer to next-step skills:
- `/start-ticket <ID>` — begin implementation
- `/failed-qe <ID>` — respond to QE feedback
- `/test-manually` — set up manual QA data

---

## Reminders
- Invoke `aha` bare first — credentials come from `~/.netrc` by default. Only
  reach for env vars or `.env` if the netrc path errors and the user can't run
  `aha auth login --with-token`.
- Pass `--no-color` always; pass `--json` when you need structured fields.
- Never echo `AHA_TOKEN`, `--token` values, the contents of `~/.netrc`, or any
  line of `.env` back to the user or to logs.
- Review is read-only: do NOT switch branches, push commits, or modify code in this
  skill. The user can chain `/start-ticket` afterwards if they want to act on the
  review.
- If the CLI returns 401/403, stop and prompt for fresh credentials — do not retry in
  a loop.
