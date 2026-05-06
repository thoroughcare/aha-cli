# Recipes

Task-oriented examples. Pair with `aha <command> --help` for full flags.

## See what's in flight in the current release

```sh
aha backlog --release TC-R-15
```

JSON-piped variant for scripting:

```sh
aha backlog --release TC-R-15 --json | jq '.releases[].epics[].features[]
  | select(.complete == false)'
```

## Find features assigned to me

```sh
aha features list --assignee "$(git config user.email)"
```

## Show everything for one feature

```sh
aha features show TC-1109
```

This fans out in parallel to fetch requirements + comments + todos.
Each todo gets a per-id GET so its `body` and `attachments` are
included alongside its comments — none of which are returned by the
list endpoint. Bounded at 3 concurrent requests to stay under Aha!'s
~5 req/sec rate limit.

## Get the URL of a single attachment for hand-off to a browser

```sh
aha features show TC-1109 --json \
  | jq -r '.todos[].todo.attachments[] | select(.file_name == "diagram.png") | .download_url'
```

Open the URL in a browser tab where you're logged into Aha! — that
session cookie is what the download endpoint actually checks.
`aha attachments download <id>` is wired up but currently runs into the
same access_denied; see the README's "Attachment downloads — known
limitation" section.

## Find every file or image attached to a feature

```sh
aha features show TC-1109 --json | jq '
  [
    .comments[]?.attachments[]?,
    .todos[]?.todo.attachments[]?,
    .todos[]?.comments[]?.attachments[]?
  ]
  | map({file_name, content_type, file_size, download_url})'
```

Three sources of attachments roll up here: feature-level comments,
todos themselves (via the per-task GET), and todo comments.

## Pull every todo body for a feature

```sh
aha features show TC-1109 --json \
  | jq -r '.todos[] | "## \(.todo.name)\n\n\(.todo.body // "(no body)")\n"'
```

## See what's recently moved

```sh
aha features list --updated-since 2026-04-01
```

## Pull the IDs of all features in a product, for scripting

```sh
aha features list --product TC --json | jq -r '.[].reference_num'
```

## Check oncall-style: what's still open in this release?

```sh
aha backlog --release TC-R-15 --json \
  | jq -r '.releases[].epics[].features[] | select(.complete == false)
           | "[\(.status)] \(.reference_num) \(.name)"'
```

## Manage credentials across multiple Aha! workspaces

Each `subdomain.aha.io` gets its own netrc entry. To switch:

```sh
# Save credentials for two accounts.
printf '%s' "$TCARE_TOKEN" | aha auth login --with-token --subdomain tcare
printf '%s' "$OTHER_TOKEN" | aha auth login --with-token --subdomain other

# Pick one for a single command:
aha --subdomain other features list

# Or set the default for this shell:
export AHA_COMPANY=other
aha features list
```

## Use it from an AI agent / script

Because stdout-is-a-pipe defaults to JSON, an agent shell tool gets clean
structured output without any flag:

```text
$ aha products list
[
  {
    "id": "...",
    "reference_prefix": "TC",
    "name": "Roadmap"
  },
  ...
]
```

For schemas / typed output: every `list` and `show` command's JSON shape
mirrors the underlying Aha! API response, with snowflake IDs typed as
strings (so they round-trip through `jq` without precision loss).
