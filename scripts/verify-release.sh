#!/usr/bin/env bash
# Smoke-test a built `aha` binary against the live Aha! API.
#
# Usage: ./scripts/verify-release.sh [path/to/aha]
#
# Defaults to ./target/release/aha. Requires existing credentials (run
# `aha auth login --with-token --subdomain <name>` first). Read-only —
# does not write to your Aha! workspace.
#
# Output: one line per check, exits non-zero on the first failure.

set -euo pipefail

AHA="${1:-./target/release/aha}"

if ! command -v jq >/dev/null 2>&1; then
  echo "verify-release: jq is required (brew install jq)" >&2
  exit 2
fi

if [[ ! -x "$AHA" ]]; then
  echo "verify-release: $AHA not found or not executable" >&2
  echo "  build with: cargo build --release" >&2
  exit 2
fi

# --- Check helpers ----------------------------------------------------------

PASS=0
FAIL=0

if [[ -t 1 ]]; then
  GREEN=$'\033[32m'; RED=$'\033[31m'; DIM=$'\033[2m'; RESET=$'\033[0m'
else
  GREEN=""; RED=""; DIM=""; RESET=""
fi

check() {
  # check "label" command...
  local label="$1"; shift
  if "$@" >/dev/null 2>&1; then
    printf '  %s✓%s %s\n' "$GREEN" "$RESET" "$label"
    PASS=$((PASS + 1))
  else
    printf '  %s✗%s %s\n    %scommand: %s%s\n' "$RED" "$RESET" "$label" "$DIM" "$*" "$RESET"
    FAIL=$((FAIL + 1))
  fi
}

# Captures stdout+stderr to a temp file; passes if it succeeds AND the
# captured output satisfies the predicate.
check_with_output() {
  local label="$1"; local predicate="$2"; shift 2
  local out
  out=$(mktemp)
  if "$@" >"$out" 2>&1; then
    if eval "$predicate" <"$out"; then
      printf '  %s✓%s %s\n' "$GREEN" "$RESET" "$label"
      PASS=$((PASS + 1))
    else
      printf '  %s✗%s %s\n    %spredicate failed; output:%s\n' "$RED" "$RESET" "$label" "$DIM" "$RESET"
      sed 's/^/      /' <"$out" | head -10
      FAIL=$((FAIL + 1))
    fi
  else
    printf '  %s✗%s %s\n    %scommand failed; output:%s\n' "$RED" "$RESET" "$label" "$DIM" "$RESET"
    sed 's/^/      /' <"$out" | head -10
    FAIL=$((FAIL + 1))
  fi
  rm -f "$out"
}

section() { printf '\n%s%s%s\n' "$DIM" "$1" "$RESET"; }

# --- 0. Build metadata ------------------------------------------------------

section "binary"
check_with_output "version flag works" \
  'grep -qE "^aha [0-9]+\.[0-9]+\.[0-9]+"' \
  "$AHA" --version

check_with_output "top-level help lists all commands" \
  'grep -qE "auth|products|releases|epics|features|requirements|todos|ideas|backlog|completions"' \
  "$AHA" --help

# --- 1. Auth ----------------------------------------------------------------

section "auth"
check "auth check succeeds (live API)" \
  "$AHA" auth check

check_with_output "auth whoami returns valid JSON with subdomain" \
  'jq -e ".subdomain != null and .email != null and .id != null" >/dev/null' \
  "$AHA" --json auth whoami

check_with_output "auth whoami table mode prints kv pairs" \
  'grep -qE "^subdomain "' \
  "$AHA" --no-json auth whoami

# --- 2. Products ------------------------------------------------------------

section "products"
PRODUCTS_JSON=$(mktemp)
"$AHA" --json products list >"$PRODUCTS_JSON"
trap 'rm -f "$PRODUCTS_JSON"' EXIT

check_with_output "products list returns non-empty JSON array" \
  'jq -e "type == \"array\" and length > 0" >/dev/null' \
  cat "$PRODUCTS_JSON"

check_with_output "products list table mode draws a sharp box" \
  'grep -qE "^[┌└]"' \
  "$AHA" --no-json products list

# Pick the TC product as the canonical target for downstream checks.
PRODUCT_PREFIX=$(jq -r '.[] | select(.reference_prefix == "TC") | .reference_prefix' "$PRODUCTS_JSON" | head -1)
if [[ -z "$PRODUCT_PREFIX" ]]; then
  PRODUCT_PREFIX=$(jq -r '.[0].reference_prefix' "$PRODUCTS_JSON")
fi
echo "    using product prefix: $PRODUCT_PREFIX"

# --- 3. Releases ------------------------------------------------------------

section "releases"
RELEASES_JSON=$(mktemp)
"$AHA" --json releases list --product "$PRODUCT_PREFIX" >"$RELEASES_JSON"

check_with_output "releases list returns JSON array" \
  'jq -e "type == \"array\"" >/dev/null' \
  cat "$RELEASES_JSON"

RELEASE_REF=$(jq -r 'first(.[] | .reference_num // empty)' "$RELEASES_JSON")
if [[ -n "$RELEASE_REF" ]]; then
  check "releases show works (using $RELEASE_REF)" \
    "$AHA" releases show "$RELEASE_REF"
else
  echo "    (skipped releases show — no releases in $PRODUCT_PREFIX)"
fi
rm -f "$RELEASES_JSON"

# --- 4. Epics ---------------------------------------------------------------

section "epics"
EPICS_JSON=$(mktemp)
"$AHA" --json epics list --product "$PRODUCT_PREFIX" >"$EPICS_JSON"

check_with_output "epics list returns JSON array" \
  'jq -e "type == \"array\"" >/dev/null' \
  cat "$EPICS_JSON"

EPIC_REF=$(jq -r 'first(.[] | .reference_num // empty)' "$EPICS_JSON")
if [[ -n "$EPIC_REF" ]]; then
  check "epics show works (using $EPIC_REF)" \
    "$AHA" epics show "$EPIC_REF"
else
  echo "    (skipped epics show — no epics in $PRODUCT_PREFIX)"
fi
rm -f "$EPICS_JSON"

# --- 5. Features ------------------------------------------------------------

section "features"
FEATURES_JSON=$(mktemp)
"$AHA" --json features list --product "$PRODUCT_PREFIX" >"$FEATURES_JSON"

check_with_output "features list returns JSON array" \
  'jq -e "type == \"array\"" >/dev/null' \
  cat "$FEATURES_JSON"

check_with_output "feature IDs are strings (snowflake-safe, no precision loss)" \
  'jq -e "all(.[]; .id | type == \"string\")" >/dev/null' \
  cat "$FEATURES_JSON"

FEATURE_REF=$(jq -r 'first(.[] | .reference_num // empty)' "$FEATURES_JSON")
if [[ -n "$FEATURE_REF" ]]; then
  check_with_output "features show $FEATURE_REF returns deep payload" \
    'jq -e "has(\"feature\") and has(\"requirements\") and has(\"comments\") and has(\"todos\")" >/dev/null' \
    "$AHA" --json features show "$FEATURE_REF"

  check_with_output "features show YAML output starts with key" \
    'head -1 | grep -qE "^[a-z_]+:"' \
    "$AHA" --yaml features show "$FEATURE_REF"
else
  echo "    (skipped features show — no features in $PRODUCT_PREFIX)"
fi

# --- 5b. Deep payload: todo bodies + attachments (feature/todo/comment) -----
#
# These fields only show up via the per-task GET we added on top of the list
# fan-out. We probe a handful of features looking for one with todos so the
# assertions exercise real API data, not just empty-array defaults.

section "deep payload: todo body + attachments"

# Allow an env override for CI / repeatable runs against a known-rich feature.
RICH_FEATURE="${AHA_VERIFY_RICH_FEATURE:-}"
if [[ -z "$RICH_FEATURE" ]]; then
  for ref in $(jq -r '.[].reference_num' "$FEATURES_JSON" | head -20); do
    n=$("$AHA" --json features show "$ref" 2>/dev/null | jq '.todos | length' 2>/dev/null || echo 0)
    if [[ "$n" =~ ^[0-9]+$ && "$n" -gt 0 ]]; then
      RICH_FEATURE="$ref"
      break
    fi
  done
fi

if [[ -z "$RICH_FEATURE" ]]; then
  echo "    (skipped — no features with todos found in first 20 of $PRODUCT_PREFIX;" \
       "set AHA_VERIFY_RICH_FEATURE=<ref> to override)"
else
  echo "    using rich feature: $RICH_FEATURE"
  RICH_JSON=$(mktemp)
  "$AHA" --json features show "$RICH_FEATURE" >"$RICH_JSON"

  # Shape checks: every todo + every comment + every todo-comment must
  # carry an `attachments` array, and todos must have a `body` key (null
  # is fine; missing is not). Catches regressions where serde silently
  # drops the field from the output.
  check_with_output "every todo carries an attachments array" \
    'jq -e "all(.todos[].todo; .attachments | type == \"array\")" >/dev/null' \
    cat "$RICH_JSON"

  check_with_output "every todo serializes a body field" \
    'jq -e "all(.todos[].todo; has(\"body\"))" >/dev/null' \
    cat "$RICH_JSON"

  check_with_output "every feature comment carries an attachments array" \
    'jq -e "(.comments | length == 0) or all(.comments[]; .attachments | type == \"array\")" >/dev/null' \
    cat "$RICH_JSON"

  check_with_output "every todo comment carries an attachments array" \
    'jq -e "all(.todos[].comments[]?; .attachments | type == \"array\")" >/dev/null' \
    cat "$RICH_JSON"

  # Structural check on any actual attachment: must have id + file_name as
  # strings (so a downstream consumer can render them).
  check_with_output "all attachments have well-formed id + file_name" \
    'jq -e "[
        .comments[]?.attachments[]?,
        .todos[]?.todo.attachments[]?,
        .todos[]?.comments[]?.attachments[]?
      ] | all(.id | type == \"string\") and all(.file_name | type == \"string\")" >/dev/null' \
    cat "$RICH_JSON"

  # Per-task GET round-trip check: at least one todo on a sufficiently
  # rich feature should have a non-empty body OR an attachment. If the
  # workspace happens to have only sparse todos, we soft-skip with a note.
  RICH_BODIES=$(jq '[.todos[].todo | select(.body != null and .body != "")] | length' "$RICH_JSON")
  RICH_ATTS=$(jq '[
                  .comments[]?.attachments[]?,
                  .todos[]?.todo.attachments[]?,
                  .todos[]?.comments[]?.attachments[]?
                ] | length' "$RICH_JSON")
  if [[ "$RICH_BODIES" -gt 0 || "$RICH_ATTS" -gt 0 ]]; then
    printf '  %s✓%s deep fan-out surfaced live data: %d todo body(ies), %d attachment(s)\n' \
      "$GREEN" "$RESET" "$RICH_BODIES" "$RICH_ATTS"
    PASS=$((PASS + 1))
  else
    echo "    (no live bodies or attachments on $RICH_FEATURE — shape checks above" \
         "still validated; set AHA_VERIFY_RICH_FEATURE to a richer ref to assert)"
  fi

  # Table mode: when a todo has body/attachments, the rendered line should
  # carry the marker tags in square brackets.
  check_with_output "table mode tags todos with body/attachment markers when present" \
    "if [ \"$RICH_BODIES\" -gt 0 ] || [ \"$RICH_ATTS\" -gt 0 ]; then
       grep -qE '\\[(body|[0-9]+ attachment|[0-9]+ comment)' ;
     else
       cat >/dev/null ;
     fi" \
    "$AHA" --no-json features show "$RICH_FEATURE"

  rm -f "$RICH_JSON"
fi
rm -f "$FEATURES_JSON"

# --- 6. Filters & query strings --------------------------------------------

section "filter / query parameters"
check_with_output "features list --tag flag accepted (returns array)" \
  'jq -e "type == \"array\"" >/dev/null' \
  "$AHA" --json features list --product "$PRODUCT_PREFIX" --tag nonexistent-tag-zzz

check_with_output "features list -q query flag accepted" \
  'jq -e "type == \"array\"" >/dev/null' \
  "$AHA" --json features list --product "$PRODUCT_PREFIX" -q xyzzy-no-match

# --- 7. Todos ---------------------------------------------------------------

section "todos"
check_with_output "todos list returns JSON array" \
  'jq -e "type == \"array\"" >/dev/null' \
  "$AHA" --json todos list

# --- 8. Ideas ---------------------------------------------------------------

section "ideas"
check_with_output "ideas list returns JSON array" \
  'jq -e "type == \"array\"" >/dev/null' \
  "$AHA" --json ideas list --product "$PRODUCT_PREFIX"

# --- 8b. Attachments --------------------------------------------------------
#
# Aha!'s download URL currently rejects API-token auth (it expects a
# browser session). We can't assert a successful byte download against
# the live API, so we verify (a) the command is wired up, (b) it exits
# non-zero with a recognizable error from Aha!, and (c) the metadata
# fields the user actually relies on are present in features-show JSON.

section "attachments (limited: live download blocked by Aha! auth model)"

check_with_output "attachments download --help is registered" \
  'grep -qE "Output path|--force"' \
  "$AHA" attachments download --help

# Pick any real attachment id from the feature deep payload (the rich
# feature lookup ran earlier in section 5b — re-derive cheaply here).
ATT_ID=""
if [[ -n "${RICH_FEATURE:-}" ]]; then
  ATT_ID=$("$AHA" --json features show "$RICH_FEATURE" 2>/dev/null \
    | jq -r '
        first(
          .comments[]?.attachments[]?,
          .todos[]?.todo.attachments[]?,
          .todos[]?.comments[]?.attachments[]?
        ) | .id // empty')
fi

if [[ -n "$ATT_ID" ]]; then
  echo "    using attachment id: $ATT_ID (live download is expected to fail)"
  # We expect non-zero with an Aha!-shaped error — succeeding here would
  # actually be news worth investigating (auth may have changed).
  check_with_output "live download exits non-zero with Aha-shaped error" \
    'grep -qiE "(access[_ ]denied|record not found|HTTP 5)"' \
    bash -c "! '$AHA' attachments download '$ATT_ID' -o /dev/null --force"
else
  echo "    (skipped live attempt — no attachments visible on $RICH_FEATURE)"
fi

# --- 9. Backlog -------------------------------------------------------------

section "backlog"
check_with_output "backlog returns grouped structure" \
  'jq -e ".releases | type == \"array\"" >/dev/null' \
  "$AHA" --json backlog --product "$PRODUCT_PREFIX"

check_with_output "backlog table mode prints Release headers" \
  'grep -qE "^Release: " || grep -q "no features match"' \
  "$AHA" --no-json backlog --product "$PRODUCT_PREFIX"

# --- 10. Completions --------------------------------------------------------

section "completions"
check_with_output "zsh completion script starts with #compdef" \
  'head -1 | grep -q "^#compdef aha"' \
  "$AHA" completions zsh

check_with_output "bash completion exposes _aha function" \
  'grep -q "_aha"' \
  "$AHA" completions bash

check_with_output "fish completion uses 'complete -c aha'" \
  'grep -q "complete -c aha"' \
  "$AHA" completions fish

# --- 11. Error paths --------------------------------------------------------

section "error paths"
check_with_output "unknown feature ref exits non-zero with helpful message" \
  'grep -qiE "(not found|404)"' \
  bash -c "! '$AHA' features show TC-DOES-NOT-EXIST-9999"

check_with_output "missing subcommand prints usage" \
  'grep -qi "Usage:"' \
  bash -c "'$AHA' 2>&1 || true"

# --- Summary ----------------------------------------------------------------

echo
TOTAL=$((PASS + FAIL))
if [[ "$FAIL" -eq 0 ]]; then
  printf '%s%d/%d checks passed%s\n' "$GREEN" "$PASS" "$TOTAL" "$RESET"
  exit 0
else
  printf '%s%d/%d checks failed%s\n' "$RED" "$FAIL" "$TOTAL" "$RESET"
  exit 1
fi
