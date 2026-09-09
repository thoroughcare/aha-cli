---
name: review-ci
description: "Review a repo's CI — its GitHub Actions workflows in
  .github/workflows/ — against a good-practices checklist, and author new
  workflows that follow it: least-privilege permissions, third-party action
  pinning, dependency caching, concurrency cancellation, job timeouts,
  secret/OIDC handling, reproducible tool versions and pinned runner images,
  shell run-step error handling and portability, scheduled (cron) workflow
  pitfalls, and failure observability. Read-only by default — reports findings
  by category with proposed fixes, waits for approval, and never breaks a
  working pipeline to satisfy a rule. Use this WHENEVER the user mentions CI, a
  GitHub Actions workflow, or .github/workflows — do not answer from memory
  instead. TRIGGER when: they ask to review/audit/harden/improve CI or a
  workflow, set up CI for a repo, add caching/permissions/concurrency/timeouts,
  pin actions to SHAs, add or review a scheduled/cron workflow, review a shell
  script that CI runs, \"is our CI following best practices\", or \"why is CI
  slow\"; or type /review-ci. SKIP when: a test is red on a PR and they just
  want it green — that is /fix-tests, not this skill."
---

# Review CI

Review and harden a repo's GitHub Actions CI against a good-practices checklist,
and scaffold new workflows that follow it. Reports findings, proposes minimal
diffs, waits for approval before editing. Never loosen or break a working
pipeline just to satisfy a checklist item — flag the tension and let the user
decide.

$ARGUMENTS

---

## Phase 1: Parse the argument and set the mode

The argument can be a **scope**, an **instruction**, or a **request to author**.

- **Audit mode** (default) — no argument, or a scope: a workflow file
  (`.github/workflows/ci.yml`), a topic ("permissions", "caching", "our deploy
  workflows"), or "review our CI". Review existing workflows against the checklist.
- **Authoring mode** — the user asks to *create* CI ("set up CI for this repo",
  "add a lint workflow"). Scaffold a new workflow that satisfies the checklist and
  matches the repo's stack and existing conventions.
- **Instruction** — an explicit directive ("only check action pinning", "just add
  concurrency cancellation everywhere") overrides the default checklist coverage.

If no argument is given and the repo has many workflows, default to a full audit
but report the highest-severity findings first; offer to narrow to one workflow.

---

## Phase 2: Discover the CI and the stack

- List the workflows: `.github/workflows/*.yml` and `*.yaml`. Read each in full.
- Note the repo's stack and toolchain (the checklist is stack-agnostic but the
  *fixes* aren't): language + version files (`.ruby-version`, `.tool-versions`,
  `package.json` engines, `mix.exs`, `Cargo.toml`, `.python-version`), the
  package manager, and how tests/lint are actually run.
- Read the repo's `CLAUDE.md` / `AGENTS.md` / `README` for CI conventions,
  required checks, and any deploy/secret policy. **Repo conventions win** over a
  generic default when they conflict.
- Record, per workflow: triggers (`on:`), `permissions:` (top-level and per-job),
  every `uses:` action + its ref, cache usage, `concurrency:`, `timeout-minutes`,
  matrices, and how secrets are referenced.

---

## Phase 3: Evaluate against the checklist

For each workflow, check every item below. Record each as **OK**, **FINDING**
(with severity), or **N/A**. Prefer the cheapest check that proves it.

### 3a. Security / least privilege  *(highest severity)*
- **Permissions** — a top-level `permissions:` is set and defaults minimal
  (`contents: read`, or `{}`); jobs elevate only the scopes they need
  (e.g. `pull-requests: write`, `id-token: write`). A workflow with no
  `permissions:` block inherits the repo default (often write-all) — flag it.
- **`pull_request_target` / fork PRs** — flag any `pull_request_target` (or
  `workflow_run`) job that checks out and runs untrusted PR code, or exposes
  secrets to it. This is the highest-risk pattern; call it out explicitly.
- **Secrets hygiene** — secrets come from `secrets.*`/env, are never `echo`ed or
  interpolated into a `run:` in a way that logs them, and aren't passed to
  untrusted steps. Prefer OIDC (`id-token: write` + cloud role) over long-lived
  cloud credentials stored as secrets.
- **Script injection** — untrusted `${{ github.event.*.title/body/... }}` values
  interpolated directly into `run:` shell. Flag; recommend passing via `env:` and
  referencing the shell variable.

### 3b. Supply chain
- **Action pinning** — third-party actions (anything not `actions/*` or a trusted
  first-party org) pinned to a **full commit SHA**, not a moving tag. `actions/*`
  pinned at least to a stable major tag (SHA is better). Flag `@main`/`@master`.
- **Dependabot** — `.github/dependabot.yml` covers the `github-actions` ecosystem
  so pins get updated. Flag if actions are pinned but never updated.

### 3c. Efficiency
- **Concurrency** — a `concurrency:` group (e.g. keyed on ref/workflow) with
  `cancel-in-progress: true` for PR/push workflows, so superseded runs are
  cancelled. (Don't cancel-in-progress on deploy/release workflows.)
- **Caching** — dependency caching present and correctly keyed (the `setup-*`
  action's built-in cache, or `actions/cache` keyed on the lockfile hash).
- **Filters** — `paths`/`paths-ignore` or `branches` filters where a workflow
  needn't run on every change (weigh against required-check semantics).
- **Timeouts** — every job sets a sane `timeout-minutes` so a hung step can't burn
  the full 6h default.
- **Matrices** — matrices are sized sensibly; `fail-fast` set intentionally
  (usually `true` for PR gates, `false` when you want all cells' results).

### 3d. Reliability / reproducibility
- **Pinned toolchains** — language/tool versions pinned (reading `.tool-versions`
  / `.ruby-version` / etc. rather than `latest`), and runner images not silently
  drifting where it matters.
- **Versioned `runs-on:`** — prefer a pinned runner label (`ubuntu-24.04`,
  `macos-15`) over `ubuntu-latest`/`macos-latest`. GitHub migrates the `-latest`
  alias to a new image on its own schedule, which changes preinstalled tool versions
  under a pipeline that nobody touched. Pinning turns that into a deliberate,
  reviewable bump. (Weigh against staleness: a pinned image eventually reaches
  end-of-life, so it needs an owner — Dependabot's `github-actions` ecosystem does
  not bump runner labels.)
- **Deterministic installs** — `npm ci` / `bundle install --frozen` / `mix deps.get`
  against a committed lockfile rather than unpinned resolution.

### 3e. Observability / maintainability
- **Failure artifacts** — logs, screenshots, or coverage uploaded on failure
  (`if: failure()`) so red runs are debuggable without re-running.
- **Clear names** — jobs/steps have descriptive `name:`s; the workflow name maps to
  a required status check the repo actually gates on. The top-level `name:` is what
  humans read in the **Actions** sidebar, so it should be a readable description
  ("Weekly check on the vendored plugins"), not a repeat of the filename slug
  (`check-vendored`). Flag `name:` values that are just the filename.
- **DRY** — duplicated job setup across workflows extracted into a reusable
  workflow (`workflow_call`) or composite action.

### 3f. Shell / run-step reliability
The shell inside `run:` blocks (and standalone `bin/ci/*.sh` scripts) is where a
job most often goes **green while it actually failed**. Check:
- **No masked failures** — a `run:` block must not end on a command that always
  succeeds (a trailing cleanup, `... || true`, or a stray `echo`) placed after a
  critical command, or the step exits 0 even when the critical command failed.
  Capture the real status first: `status=$?; <cleanup>; exit "$status"`.
- **Propagate real exit codes** — don't blanket-wrap with `|| exit 1`; the tool's
  own exit code is more diagnostic. For specialized tools (`curl`, `flyctl`,
  `python`) capture `$?`, log a helpful message, then exit with that code — but
  don't guard trivial builtins (`cd`, `date`, `ls`).
- **`set -euo pipefail` is an anti-pattern — flag it wherever it appears.** Not just
  in `run:` blocks (where it is also *redundant*, since GitHub already runs them
  under `bash -e -o pipefail`) but in standalone scripts too. It reads like a safety
  net and is not one: `set -e` silently does nothing inside command substitutions,
  `&&`/`||` chains, and most function-call contexts; `set -u` breaks on legitimately
  unset variables and on empty-array expansion in bash < 4.4; `pipefail` turns a
  benign `SIGPIPE` (`... | head -1`) into a failure. See
  [BashPitfalls](https://mywiki.wooledge.org/BashPitfalls#set_-euo_pipefail).
  The fix is explicit error handling: check every command whose failure matters with
  `|| die "..."`, and leave the rest alone. Prefer starting the continuation line
  with `||` so it reads as a continuation rather than a new command:
  ```bash
  latest=$(gh api "repos/${repo}/commits" --jq '.[0].sha') \
    || die "could not reach the API"
  ```
  Be specific about the consequence when you flag it, because "it looked handled" is
  the whole failure mode — an unchecked command that returns empty output on failure
  is indistinguishable from a legitimately empty result, which is how a checker ends
  up taking the *wrong* mutating action (see **Fail closed** below).
- **Guard must-succeed commands** — chained setup/pre-deploy commands (`aws s3 cp`,
  `flyctl secrets import`) must each fail the step; otherwise a later success
  (e.g. `flyctl deploy`) masks the earlier failure and the app ships misconfigured.
- **Fail closed, not open** — an unguarded check (`curl -sf ...`, `gh api ...`) that
  falls through on error can misread a network/API failure as "nothing found" and take
  the wrong mutating action (e.g. create a duplicate, or *close* an issue that should
  have stayed open). Handle the error path.
- **Portable, reviewable shell** — for standalone scripts a CI job calls, and for any
  `run:` block long enough to reason about:
  - **`#!/bin/bash` and a `.bash` extension** when the script uses Bash-isms — `.sh`
    implies Bourne-compatible, and `#!/usr/bin/env bash` picks whichever Bash is first
    on `PATH`. Hard-coding `/bin/bash` means the script is exercised against the
    *system* Bash, so version assumptions fail on the author's machine rather than
    someone else's.
  - **Know the target Bash version.** If maintainers run macOS, the system Bash is
    **3.2**: no `mapfile`, no associative arrays, no `${var,,}`. State the target in a
    header comment.
  - **No GNU-only flags in a script that must run on macOS.** `head --lines`,
    `cut --fields`, `sed -i` (no arg), `date -d` and `readlink -f` are GNU-only; BSD
    userland rejects them. `jq`, `gh`, `grep` and `git` ship their own parsers and
    accept long options on both, so prefer long forms *there* for self-documentation.
    Better still, do the parsing in `jq` and skip the `head`/`cut` pipeline.
  - **`[[ ]]` over `[ ]`**, `==` over `=` in tests, and braced + quoted `"${var}"`.
  - **A `usage()`/`help()` function** for `--help`, rather than `sed`-ing the header
    comment — clever extraction breaks the moment someone edits a comment line.
  - **Heredocs (or plain `echo`) over stacked `printf` calls** for multi-line output.

### 3g. Scheduled (`cron`) workflows
A `schedule:` trigger has failure modes no other trigger has, and they are all silent
— a broken scheduled workflow looks exactly like a healthy one with nothing to report.
For any workflow with a `schedule:` trigger, check:
- **Testability.** A scheduled workflow only ever runs from the **default branch**, so
  it cannot be exercised from a PR branch — the change is untestable until merged.
  Require *both*: a `workflow_dispatch` trigger so it can be run on demand after merge,
  and (when the logic is non-trivial) that logic in a script the author can run locally
  rather than inline YAML. Flag a cron workflow with substantial inline `run:` logic and
  no local entry point: it ships untested by construction.
- **Local-time comment.** `cron` is UTC-only, with no DST handling. A bare
  `'0 9 * * 1'` makes every future reader do the conversion. Expect a comment with the
  local equivalent: `# Mondays at 09:00 UTC (= 05:00 EDT / 04:00 EST)`.
- **Best-effort timing.** Scheduled runs can be delayed (or dropped) when GitHub is
  under load, and the `*/5`-style high-frequency schedules are the first to suffer.
  Flag a schedule whose correctness depends on firing on time; anything time-critical
  needs an external trigger.
- **60-day auto-disable.** GitHub disables scheduled workflows in a repo with no
  activity (push, PR, comment) for 60 days. Low-traffic repos — tooling, infra,
  archives — will hit this. Flag it, and check whether the workflow's *own* output
  (filing/commenting on a tracking issue) counts as activity and partly self-sustains.
- **Concurrency without cancellation.** A cron workflow that writes something (an
  issue, a commit, a release) should serialize with a `concurrency:` group so a manual
  dispatch can't race the scheduled run, but should **not** set
  `cancel-in-progress: true` — cancelling mid-run can drop the write half-done. This is
  the opposite of the PR-gate default in 3c.

### 3h. Tooling (advisory)
- If `actionlint` is available (`which actionlint`), run it on the workflows and
  fold its output into the findings. If not installed, note it as a suggested
  local/CI check rather than a blocker.
- If `shellcheck` is available, run it on standalone `bin/` scripts (and, where
  practical, the shell inside `run:` blocks) — it catches the masking, quoting, and
  `set` pitfalls in 3f. Otherwise suggest it as a lint step.
- If `zizmor` is available, run it for a security-focused pass; otherwise mention
  it as an option. Never install tools without asking.

---

## Phase 4: Report findings

Present a numbered list grouped by workflow, ordered **most severe first**
(Security → Supply chain → Efficiency → Reliability → Shell/run-step → Scheduled →
Observability).
For each:
file:line, category, what's wrong, why it matters, and the proposed fix.

```
.github/workflows/ci.yml
  1. [SECURITY] no top-level `permissions:` — inherits repo default (write-all).
     Fix: add `permissions: contents: read` at top; elevate per-job as needed.
  2. [SUPPLY-CHAIN] uses `some/action@main` (L42) — moving ref.
     Fix: pin to the commit SHA of the release you want, add a `# vX.Y.Z` comment.
  3. [EFFICIENCY] no `concurrency:` — superseded PR runs keep running.
     Fix: add a concurrency group keyed on the ref with cancel-in-progress.
  4. [SHELL] deploy step ends on `flyctl ... destroy || true` (L88) after the
     deploy — a failed deploy exits 0 and the step goes green.
     Fix: `status=$?; flyctl ... destroy || true; exit "$status"`.
  5. [SHELL] `set -euo pipefail` at bin/ci/release.sh:3 — reads as a safety net
     but `set -e` doesn't fire inside the `$( )` on L22, so a failed API call
     there returns empty and the script proceeds as if there was nothing to do.
     Fix: drop the `set` line; add `|| die "..."` to the calls that matter.
  6. [SCHEDULED] cron-only trigger with 60 lines of inline `run:` logic — only
     runs from the default branch, so it ships untested.
     Fix: add `workflow_dispatch`, move the logic to a locally-runnable script.
```

Do not edit yet. If a "fix" would weaken security or break the pipeline (e.g.
adding a path filter to a required check), say so and let the user decide.

---

## Phase 5: Apply fixes with approval

Ask which findings to apply. For each approved item:
- Use `Edit` (not `Write`) — minimal, targeted diffs; preserve existing style,
  ordering, and comments.
- For action pins, never write a SHA from memory — resolve it against the API:
  `gh api repos/<owner>/<repo>/commits/<tag> --jq .sha` (for an annotated tag use
  `.../git/refs/tags/<tag> --jq .object.sha`). Keep a trailing `# vX.Y.Z` comment so
  the human-readable version stays visible.
- Re-validate after editing: `actionlint` if available, otherwise confirm the YAML
  parses and the changes are well-formed.
- Follow the repo's staging/commit conventions (check CLAUDE.md/AGENTS.md); don't
  `git add` if the repo asks you not to.

**Authoring mode:** scaffold the new workflow satisfying every applicable
checklist item from the start — minimal `permissions:`, pinned actions, caching,
`concurrency`, `timeout-minutes` — and match the repo's existing workflow naming
and structure. Show the full file before writing it.

---

## Phase 6: Wrap up

Report what changed (file:line list), what was left unfixed and why, and any
follow-ups that belong in a separate PR (e.g. adding `dependabot.yml`, extracting
a reusable workflow, enabling a required status check in repo settings — which is
a GitHub settings change, not a file edit).

---

## Reminders

- **A green step can still be a failed step.** The most damaging CI bug is a job
  that exits 0 while its real work failed — check `run:` blocks don't mask non-zero
  exits (trailing `|| true`, cleanup as the last line, unpropagated exit codes).
- **Stack-agnostic checklist, stack-specific fixes.** The same practices apply to
  Rails, Elixir/Phoenix, Ruby, Node, Rust, and Python repos, but the concrete
  cache key, setup action, and install command differ per stack — match the repo.
- **A checklist item is a default, not a mandate.** If satisfying one risks breaking
  or weakening a working pipeline, flag the trade-off and let the user decide rather
  than forcing the change.
