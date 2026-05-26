# aha-cli

Browse the ThoroughCare Aha! workspace from your terminal.

A single static Rust binary. Auto-detects whether stdout is a terminal:
humans get tables, pipes and AI agents get JSON.

## Install

### Toolchain

The required Rust version is pinned in [`.tool-versions`](.tool-versions) (matches the
`rust-version` in `Cargo.toml`). If you use [asdf](https://asdf-vm.com/) or
[mise](https://mise.jdx.dev/), they'll pick it up automatically:

```sh
# asdf
asdf plugin add rust
asdf install

# mise
mise install
```

Otherwise install the version listed in `.tool-versions` via
[rustup](https://rustup.rs/):

```sh
rustup toolchain install 1.86.0
rustup override set 1.86.0
```

### Build

From this repo:

```sh
cargo install --path .
```

(Brew tap formula and pre-built release binaries land later.)

## Authentication

Every command (including read-only `list` / `show`) needs a personal
API token. **Generate one before doing anything else:**

> https://<your-subdomain>.aha.io/settings/personal/developer

The CLI looks for credentials in this order:

1. `--token` / `--subdomain` flags (rarely used directly).
2. `AHA_TOKEN` + `AHA_COMPANY` env vars — the recommended setup for
   scripts, CI, and one-off shells.
3. A `.netrc` entry written by `aha auth login --with-token`.

Pick one. Env vars and `.netrc` can coexist; the env vars win when
both are set.

### Option A — env vars (recommended for scripts)

```sh
export AHA_COMPANY=tcare
export AHA_TOKEN='aha_pat_...'   # keep this out of shell history!

aha auth check
```

### Option B — persist to `.netrc` once

```sh
printf '%s' "$AHA_TOKEN" | aha auth login --with-token --subdomain tcare
aha auth check
```

The interactive browser-based OAuth flow (`aha auth login` without
`--with-token`) lands once an OAuth app is registered on the Aha!
account; for now `--with-token` is the supported path.

## Quickstart

```sh
# After authenticating (see above):
aha auth check
aha backlog
```

## Read commands

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

## Write / edit commands

| Command                                        | What it does |
| ---------------------------------------------- | ------------ |
| `aha features create --product <P> --name <N>` | Create a feature. |
| `aha features edit <ref> [field flags]`        | Update fields; `--add-tag` / `--remove-tag` do a GET+PUT merge. |
| `aha features comment <ref>`                   | Post a comment on a feature. |
| `aha requirements edit <ref> [field flags]`    | Update a requirement; `--editor` pre-fills the existing description. |
| `aha requirements comment <ref>`               | Post a comment on a requirement. |
| `aha todos create --on <ref> --name <N>`       | Create a to-do scoped to a feature / requirement / release / epic. |
| `aha todos edit <id>`                          | Update fields on an existing to-do. |
| `aha todos done <id>` / `reopen <id>`          | Convenience: flip status to completed / pending. |

Every write command supports `--dry-run` (prints the request without
sending), `--yes` / `-y` (skip the TTY prompt), and one of
`--body / --body-file <path|-> / --editor` for free-form bodies.
Non-TTY shells must pass `--yes` explicitly — agents can't write
without opting in.

> **Known Aha! API limitation: to-do status is effectively read-only.**
> `PUT /tasks/<id>` silently no-ops `task.status` — the server returns
> 200 OK but the state never moves. `aha todos done` / `reopen` and
> `aha todos edit --status` GET the task after the PUT and surface a
> clear error in this case. Use the Aha! web UI to flip a to-do's
> state until Aha! fixes the API (probed 2026-05-21 with
> `examples/probe_task_status.rs`).

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

### Attachment downloads

`aha attachments download <id>` works for **every attachment whose blob
is still in Aha!'s storage** — confirmed live on TC-16 / TC-18 images.
Streams into a sibling tempfile and renames on success, so a failure
never leaves a 0-byte stub or wipes a `--force` target.

What about the failures? After probing it carefully (see
`examples/probe_attachment.rs`), all the broken cases we've seen share
one fingerprint in the metadata:

```jsonc
{ "file_size": null, "original_file_size": null }
```

When both size fields are null, **Aha! itself no longer has the bytes**
— the metadata pointer survives in the API but the underlying file has
been purged from their storage. Every URL variant (no `?size=`,
`?size=large`, `?size=medium`, `?size=thumbnail`) returns 302 →
`/access_denied` → "Record not found (500)" for both API tokens *and*
logged-in browser sessions. There is no auth path that recovers them
from the API.

We catch this case **before issuing the download** and report it
clearly:

```
error: attachment <id> (<file_name>) is tombstoned: Aha! still serves
the metadata pointer but reports `file_size: null` and
`original_file_size: null`, which has consistently meant the blob has
been purged from their storage. The bytes are unrecoverable through any
URL we've tested (API token, browser session, every `?size=` variant —
all 302 to /access_denied). Aha! support may be able to restore from
backup if the file is critical; we can't fetch it from here.
```

If you hit a non-tombstoned attachment that still fails (file_size is
set but the URL 302s anyway — outside the pattern we've observed),
please report the attachment id; we'll widen the diagnostic.

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
