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
| `aha features show <ref>`       | Deep view: feature + requirements + comments + todos. |
| `aha requirements show <ref>`   | Show one requirement. |
| `aha todos list [--feature]`    | List todos. |
| `aha todos show <id>`           | Show one todo. |
| `aha ideas list [--product]`    | List ideas. |
| `aha ideas show <ref>`          | Show one idea. |
| `aha backlog [filters]`         | Features grouped by release → epic. |
| `aha completions <shell>`       | Print a completion script. |

Run `aha <command> --help` for full details.

## Output formats

By default, `aha` checks whether stdout is a terminal:

- **TTY** → human-readable tables.
- **non-TTY** (pipes, file redirects, AI agent shells) → JSON.

Override explicitly with `--json`, `--no-json`, or `--yaml`. `--no-color`
disables ANSI; `NO_COLOR` env var is honored.

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

See [`docs/recipes.md`](docs/recipes.md) for task-oriented examples.
