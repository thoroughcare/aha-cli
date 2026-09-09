---
name: linear
description: 'Interact with Linear.app issues via the `linear-cli` tool — view,
  list, search, create, update, comment, start, and close issues, plus branch/PR
  integration. Reads are immediate; writes
  (create/update/comment/close/delete/archive) preview first and wait for
  explicit confirmation. TRIGGER when: user asks to look at / list / search /
  create / update / comment on a Linear issue, references a Linear issue ID like
  `ENG-123` or a `linear.app/...` URL, says "what does ENG-123 say", "move
  ENG-123 to In Progress", "file a Linear issue", or types /linear [args]. SKIP
  when: the work is tracked in a different system (e.g. Aha!, Jira, GitHub
  Issues) — some repos use another tracker for product work, and Linear IDs/URLs
  are the only signal this skill applies; or the user just wants a plain git
  branch with no issue link.'
---

# Linear

Interact with Linear.app issues from the terminal using the `linear-cli` binary.

$ARGUMENTS

If the request is ambiguous (no issue ID, no clear verb), ask what the user wants to do
before running anything.

> **Linear only — not your repo's other trackers.** This skill operates exclusively on
> **Linear** issues (`ENG-123`, `linear.app/...`). If the current repo tracks product work
> in another system (Aha!, Jira, GitHub Issues, etc.), that data is the app's *domain* and
> is reached through that system's own API/client — not this skill. Only act on Linear
> issue IDs and `linear.app` URLs here.

---

## Phase 0: Preconditions — binary + auth

The binary is `linear-cli` (Rust binary, typically at `~/.cargo/bin/linear-cli`; some
docs alias it as `linear`). Invoke it as `linear-cli` — the `linear` alias may not exist.

1. **Check it's installed:** `command -v linear-cli`. If empty, do **not** try to install
   it silently — tell the user and offer the install options, then stop:
   - `cargo binstall linear-cli` (fastest, pre-built) or `cargo install linear-cli`
   - Pre-built binaries: <https://github.com/Finesssee/linear-cli/releases>

2. **Check auth once per session:** `linear-cli auth status`. A healthy result shows an
   `Auth type` of `api-key` (an older setup may still show `oauth`). If it reports no
   credentials, or a command returns `401`/`Unauthorized`, surface the error and **prompt
   the user to set a personal API key** — do **not** fall back to the browser OAuth flow:
   - Generate a key at <https://linear.app/thoroughcare/settings/account/security>, then
     store it with `linear-cli auth login` (prompts for the key interactively; add
     `--validate` to verify it before saving). For non-interactive setup:
     `printf '%s\n' "<api-key>" | linear-cli config set-key`.

   Do not retry in a loop on a 401 — stop and prompt for the API key.

   > **ThoroughCare uses personal API keys, not OAuth.** Don't run `linear-cli auth oauth`
   > for new setups. An existing OAuth session still works, but when it expires or fails,
   > re-authenticate with an API key rather than renewing OAuth.

### The CLI is the interface — MCP is the fallback

Some environments also expose Linear through MCP tools (`mcp__linear__*`). **Prefer the
CLI for every Linear read and write**: it returns issues, comments, teams and statuses as
JSON in a single call, and it's what the rest of this skill is written against.

- **Fall back to the MCP tools only when the CLI isn't installed** (`command -v linear-cli`
  is empty).
- **If the CLI is installed but a command fails, fix the CLI** — a `401` means re-auth
  (above), not a switch to MCP. A transient auth failure isn't a reason to change tools,
  and the two backends can drift in what they return.
- Common MCP equivalents, if you do need them: `get_issue`, `list_issues`, `list_comments`,
  `list_teams`, `list_issue_statuses`, `create_issue`, `save_issue`. Note that a
  description write through `save_issue` **replaces the whole body**, so fetch-modify-write
  rather than assuming a patch.

---

## Phase 1: Global flags — reach for these

Pass `--no-color` on every invocation; add the rest when they apply, so output is
clean and (for writes) safe:

- `--no-color` — always, so streamed output is readable.
- `--output json --compact` — when you need to **parse** fields programmatically; pair
  with `--fields identifier,title,state.name,...` to keep token use low.
- `--quiet` — to suppress decorative headers/tips when scripting.
- `--dry-run` — on any **write** command that supports it, to preview before committing.
- `--filter field=value` — to narrow lists (`=`, `!=`, `~=` contains; dot-paths like
  `state.name="In Progress"`; case-insensitive; repeatable, AND-combined).
- `--limit N` — cap list/search results.

Issue IDs look like `ENG-123`, `LIN-456` (team prefix + number). When the user pastes a
`https://linear.app/<workspace>/issue/ENG-123/...` URL, extract `ENG-123`.

**Never hand-build an issue URL** to quote back or drop in a PR body — the workspace slug
varies, so a constructed URL can 404. Get it from `linear-cli issues link <ID>` or the `url`
field of a JSON read.

> Linear descriptions and comments are **Markdown**. Write normal Markdown.

If you're unsure of a subcommand's exact flags, run `linear-cli <command> --help` rather
than guessing — the surface is large and versioned.

---

## Phase 2: Reads (run immediately, no confirmation)

| Goal | Command |
|------|---------|
| Current issue from git branch | `linear-cli context --no-color` (`--output json` for fields) |
| List issues | `linear-cli issues list --no-color [--filter team.key=ENG] [--filter state.name="In Progress"] [--limit 20]` |
| View an issue | `linear-cli issues get <ID> --no-color` (add `--comments` / `--history` for those) |
| Search | `linear-cli search issues "query" --no-color` |
| Open in browser | `linear-cli issues open <ID>` / print URL: `linear-cli issues link <ID>` |
| Statuses for a team | `linear-cli statuses list --no-color` (useful before an update) |

Stream the raw CLI output back so the user can see the source, then summarise: title &
state, description intent (2–3 sentences), assignee, labels, priority, and any open
comments that change scope or flag blockers.

---

## Phase 3: Writes — preview, confirm, then commit

For every write — `create`, `update`, `assign`, `comment`, `close`, `start`, `stop`,
`archive`, `delete`, `move`, `transfer` — **show a preview and wait for explicit
confirmation before running the real command.** Prefer `--dry-run` to generate the
preview where supported; otherwise show the exact command and the rendered
title/description/comment body. Do not pass `--yes` unless the user has explicitly
authorized skipping confirmation.

### 3a. Create

```
linear-cli issues create "<TITLE>" \
  --team <TEAM-KEY> \
  --description - \              # reads Markdown body from stdin (or -d "short body")
  --priority <0-4> \            # 0 none, 1 urgent, 2 high, 3 normal, 4 low
  --assignee me \               # optional
  --labels "<label>" \          # repeatable
  --no-color
```

Preview to show the user first:

```
**New Linear Issue Preview:**
- Team: <TEAM-KEY>
- Title: <TITLE>
- Priority: <name or "none">
- Assignee: <me / name / unset>
- Labels: <list or "none">
- Description (Markdown):
  <body or "none">
```

After creation, report the new identifier (e.g. `ENG-457`) and its URL
(`linear-cli issues link <ID>`).

### 3b. Update / move state

```
linear-cli issues update <ID> --state "In Progress" --no-color   # also -p, -a, -l, --due
linear-cli issues assign <ID> "Person Name"                      # shortcut for --assignee
```

**Look the state name up per team, then verify the write took.** Workflow-state names differ
by team, and passing one the team doesn't have **does not error** — it silently lands
somewhere else. Passing `"In Review"` to a team whose state is `"Review"` left a ticket
sitting in *In Progress*, and the response said so while nobody read it. So:

```bash
linear-cli statuses list --team <KEY> --no-color --output json --fields name,type,position
linear-cli issues update <ID> --state "<exact name>" --no-color
linear-cli issues get <ID> --no-color        # assert the state is what you asked for
```

Assert the returned status equals what you asked for — the read-back is the check, not the
absence of an error.

### 3c. Comment

```
linear-cli issues comment <ID> --body - --no-color   # Markdown from stdin (or -b "text")
```

Show the rendered comment body and wait for OK before posting.

### 3d. Lifecycle shortcuts

- `linear-cli issues start <ID>` — set In Progress + assign to me (add `--checkout` to
  also create/switch to the linked git branch). **Avoid it on a team with more than one
  `started`-type state:** it picks by type, not by name, so on a team whose started states
  include *Review*, *QE* and *Ready for Smoke Test* it can land the ticket in one of those —
  falsely signalling the work is already built and in QA. Check
  `linear-cli statuses list --team <KEY>` first; where there's more than one, set the state
  explicitly (3b) and assign separately (`--assignee me`). `--checkout` also fights a repo
  whose branch convention differs from the tracker's suggestion — see
  `/update-branch`.
- `linear-cli issues close <ID>` — mark Done.
- `linear-cli done` — mark the **current branch's** issue Done.
- `linear-cli issues delete <ID>` / `archive <ID>` — destructive; always preview and
  require explicit confirmation, never auto-`--yes`.

---

## Phase 4: Git integration (optional)

Only when the user wants to act on an issue, not just read it:

- `linear-cli git checkout <ID>` — create/switch to the issue's Linear-suggested branch.
- `linear-cli git branch <ID>` — print the suggested branch name.
- `linear-cli git pr <ID> [--draft]` — open a PR linked to the issue.

> Follow the current repo's own branch and PR conventions (base branch, commit-message
> style, squash vs. merge) when opening a PR for a Linear issue — check `git log` and any
> `CONTRIBUTING`/`CLAUDE.md` if unsure. Use `linear-cli git` only for Linear-tracked work.

**Know whether the repo moves ticket state for you.** Some repos wire a workflow (commonly
`.github/workflows/ready_for_review.yml`) so that opening a non-draft PR advances the ticket
on its own; others have no Linear automation at all, and a ticket worked there stays where it
is until you move it by hand. One line answers it in the repo you're actually in:

```bash
ls .github/workflows/ | grep -i 'linear\|ready_for_review'
```

Linear's own GitHub integration may separately move a ticket to an in-progress state when it
spots the branch, which makes a stalled ticket look managed. Check, rather than assuming a
transition happened — and when you do set the state by hand, verify the write (3b).

---

## Reminders

- Never echo the API key, the contents of the config file, or any credential back to the
  user or logs.
- On `401`/`Unauthorized`, stop and prompt the user to set a personal API key
  (`auth login`, or `config set-key`) — **not** OAuth — and do not retry in a loop.
- Reads run immediately; **every write previews and waits for explicit confirmation** —
  use `--dry-run` for the preview, never auto-`--yes` on deletes/archives.
