# aha-cli

Browse the ThoroughCare Aha! workspace from your terminal.

A single static Rust binary. Auto-detects whether stdout is a terminal:
humans get tables, pipes and AI agents get JSON.

## Install

From this repo:

```sh
cargo install --path .
```

(Brew tap formula and pre-built release binaries land later.)

## Quickstart

```sh
# Generate a personal API token at:
#   https://<your-subdomain>.aha.io/settings/personal/developer
# Then save it (the token is read from stdin so it never lands in shell history):
printf '%s' "$TOKEN" | aha auth login --with-token --subdomain tcare

aha auth check
aha backlog
```

The interactive browser-based OAuth flow (`aha auth login` without
`--with-token`) is wired in once an OAuth app is registered on the Aha!
account; for now `--with-token` is the supported path.

## Commands

| Command                         | What it does |
| ------------------------------- | ------------ |
| `aha auth login --with-token`   | Save credentials (token piped on stdin). |
| `aha auth check`                | Verify stored credentials. |
| `aha auth whoami`               | Print authenticated user. |
| `aha auth logout`               | Remove stored credentials. |
| `aha products list`             | List products / workspaces. |
| `aha releases list [--product]` | List releases. |
| `aha releases show <ref>`       | Show one release. |
| `aha epics list [--product] [--release]` | List epics. |
| `aha epics show <ref>`          | Show one epic. |
| `aha features list [filters]`   | List features. Filters: `--product`, `--release`, `--epic`, `--tag`, `--assignee`, `--updated-since`, `-q`. |
| `aha features show <ref>`       | Deep view: feature + requirements + comments + todos (with bodies and attachments). |
| `aha requirements show <ref>`   | Show one requirement. |
| `aha todos list [--feature]`    | List todos. |
| `aha todos show <id>`           | Show one todo with body, attachments, and comments (each with their own attachments). |
| `aha ideas list [--product]`    | List ideas. |
| `aha ideas show <ref>`          | Show one idea. |
| `aha backlog [filters]`         | Features grouped by release → epic. |
| `aha attachments download <id>` | Download an attachment to disk (see caveat below). |
| `aha completions <shell>`       | Print a completion script. |

Run `aha <command> --help` for full details.

See [`docs/recipes.md`](docs/recipes.md) for task-oriented examples.

## Output formats

By default, `aha` checks whether stdout is a terminal:

- **TTY** → human-readable tables.
- **non-TTY** (pipes, file redirects, AI agent shells) → JSON.

Override explicitly with `--json`, `--no-json`, or `--yaml`. `--no-color`
disables ANSI; `NO_COLOR` env var is honored.

### `features show` JSON shape

The deep view fans out per-feature to surface data the list endpoint
omits:

```jsonc
{
  "feature":      { ... },
  "requirements": [ ... ],
  "comments":     [ { "body": "...", "attachments": [ ... ] }, ... ],
  "todos": [
    {
      "todo": {
        "id": "...", "name": "...", "status": "...",
        "body": "free-text body, only present on the per-task GET",
        "attachments": [ { "id", "file_name", "download_url",
                           "content_type", "file_size" }, ... ]
      },
      "comments": [ { "body": "...", "attachments": [ ... ] }, ... ]
    },
    ...
  ]
}
```

`Todo.body` and `Todo.attachments` come from a per-task GET that runs in
parallel with the comment fetch — they're omitted by the list endpoint.
Bounded at 3 in-flight requests to stay under Aha!'s ~5 req/sec cap.

In table mode, todos with body/attachments/comments are tagged inline:

```
todos:
  [completed] Clinical Input review  [body; 1 attachment(s)]
  [completed] Acceptance Criteria Review  [body; 1 comment(s)]
```

### Attachment downloads — two regimes

Aha! exposes `download_url`s in two flavours, both with the same shape
(`/attachments/<id>/token/<sig>.download`):

- **Public-by-signature**: anyone with the URL gets a 200 + bytes. Most
  comment images land here. `aha attachments download <id>` works.
- **Browser-session-gated**: signed URL 302s to `/access_denied` for
  anyone who isn't logged in via the web UI — including API-token
  callers. Some todo PDFs land here.

When the gated case fires we detect the 302 (we disable redirect
following on the download client so we don't loop or chase into an
opaque 500), exit non-zero, and print:

```
error: attachment <id> is gated by Aha! browser-session ACL —
the signed download_url isn't usable with the API token.
Open this URL in a logged-in browser tab instead:
  https://<subdomain>.aha.io/attachments/<id>/token/<sig>.download
```

No partial / 0-byte file is left behind on failure (we stream into a
sibling tempfile and rename only on success).

The metadata (`download_url`, `file_name`, `content_type`, `file_size`,
`id`) is always available regardless via `aha features show <ref>` or
`aha todos show <id>` — pipe it to `jq` to drive scripts, or paste the
URL into a logged-in browser tab to fetch a gated attachment by hand.

## Authentication

Resolution order, first hit wins:

1. `--token` / `--subdomain` flags.
2. `AHA_TOKEN` / `AHA_COMPANY` env vars.
3. `~/.netrc` entry written by `aha auth login`.

The netrc entry format is standard:

```
machine <subdomain>.aha.io
  login oauth
  password <token>
```

Created with mode `0600` if new; existing permissions preserved on update.

## Shell completions

```sh
# zsh
aha completions zsh > ~/.zfunc/_aha
# bash
aha completions bash > /usr/local/etc/bash_completion.d/aha
# fish
aha completions fish > ~/.config/fish/completions/aha.fish
```

## Development

```sh
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

The integration tests run against a `wiremock::MockServer` — the binary
points at it via the undocumented `AHA_BASE_URL` env var. No live API
calls in CI.

For a manual smoke test against the real Aha!:

```sh
aha auth login --with-token --subdomain tcare < token.txt
aha products list
aha backlog
```
