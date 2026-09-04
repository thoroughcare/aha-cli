---
name: fix-tests
description: "Fix failing CI tests for an existing pull request. Pulls the
  failed test output from GitHub Actions, returns the PR to draft while the diff
  is mid-flight, switches to the branch, reproduces locally, fixes the root
  cause, adds a regression test, runs a self-review, then pushes and ends the
  draft round — handing the un-draft to code-review:address-review-comments
  where that skill is available, or doing it directly. TRIGGER when: user says
  tests are failing/red on a PR, asks to fix CI failures, pastes a failed-test
  list, asks \"why is CI failing\", or types /ci:fix-tests [PR#]. SKIP when:
  tests pass locally and CI is green — there's nothing to fix."
---

# Fix Tests

Fix failing CI tests for an existing pull request. Argument (optional): PR number
or URL. If omitted, use the current branch's PR.

$ARGUMENTS

---

## Phase 1: Identify the PR and Branch

### 1a. Resolve the PR
- If an argument is provided, extract the PR number from it (handles both `123` and a full URL).
- If no argument, detect from current branch: `gh pr view --json number,headRefName,title`.
- Get PR details: `gh pr view <NUMBER> --json title,headRefName,baseRefName,state,body`.

### 1b. Gather context
- Read the PR title and body for what the change is supposed to do.
- If the branch name encodes a ticket ID (e.g. `[A-Z]+-\d+`), note it; pull extra
  context from the issue tracker only if the repo's workflow provides a way to
  (e.g. an installed issue-tracker plugin or CLI). Otherwise work from the PR.

### 1c. Check PR status — stop unless something actually failed
- Run `gh pr checks <NUMBER>` to see which checks passed/failed.
- **Stop unless at least one check has *failed*.** `gh pr checks` exits **8 when checks
  are still pending**, which is not the same as red — and 2a below looks for a *failed*
  run, so a pending PR gives the skill nothing to work with and dead-ends. Wait for the
  run (`gh pr checks <NUMBER> --watch`) or stop; either way do it **before** 1d, so a PR
  that was never broken is never drafted.
- Also stop if the PR is merged or closed (`state` from 1a) — `gh pr ready --undo` errors
  on those.

### 1d. Return the PR to draft — and record whether you were the one who drafted it
CI is red, so the PR is not ready for review whatever its current state says. **First read
the current state, then draft only if it was ready:**
```
gh pr view <NUMBER> --json isDraft --jq .isDraft   # remember this: WAS_DRAFT
gh pr ready --undo <NUMBER>                        # only when WAS_DRAFT is false
```
Otherwise a reviewer can pick up a branch you're actively rewriting, and their comments
land on code that's already been replaced — wasted reviewer time on a diff that no longer
exists. Say so in your report; a silent state change is invisible to whoever is watching
the PR.

**`WAS_DRAFT` decides the ending, so carry it to Phase 6:**

- **`WAS_DRAFT: false`** — you drafted it, so **you own the un-draft** (Phase 6). A PR left
  in draft is never reported as shipped, and on most repos it isn't auto-reviewed either,
  so forgetting it strands the change.
- **`WAS_DRAFT: true`** — the PR was *already* a draft and you changed nothing. **Do not
  un-draft it.** It may be an unfinished draft, or another skill's round in progress (this
  skill is commonly invoked from inside `/code-review:address-review-comments`, which
  drafts for the duration of its own round). Un-drafting there announces someone else's
  half-finished work to reviewers. Leave the state alone and say so in your report.

**If you stop before Phase 6 for any reason** — nothing parseable in the logs, a root cause
you can't fix here, a judgment call you hand back to the user, or the session simply ending
— and `WAS_DRAFT` was false, then **restore the PR to ready (`gh pr ready <NUMBER>`) or say
plainly in your report that you left it in draft and why.** Every stop point below predates
this step and was harmless when the skill made no remote change; now each one can strand a
PR that was ready when you found it.

---

## Phase 2: Extract CI Failures

### 2a. Find the failed test job
```
gh run list --branch <BRANCH_NAME> --limit 5
```
Pick the most recent failed run.

### 2b. Get the failed job ID
```
gh run view <RUN_ID> --json jobs --jq '.jobs[] | select(.conclusion == "failure") | {name, databaseId}'
```

### 2c. Download and parse the logs
Save the full job log to a temp file to avoid permission issues with piped commands.
Resolve `<owner>/<repo>` from `gh repo view --json nameWithOwner --jq .nameWithOwner`:
```
gh api repos/<owner>/<repo>/actions/jobs/<JOB_ID>/logs > /tmp/ci-log.txt 2>&1
```

### 2d. Extract the failed tests
Search the log for the failure summary your test runner prints. The exact pattern
depends on the runner — for example:
- RSpec: lines beginning `rspec ./spec/...`
- Jest/Vitest: the `FAIL <path>` lines plus the per-test `✕` lines
- pytest: the `FAILED <path>::<test>` lines
- Go: `--- FAIL: <TestName>` lines

Reduce them to a unique, one-line-per-failure list of the failing test files/IDs.

### 2e. Extract the first failure's error details
Pull the failures section (message, stack trace, and diff) from the log so you can
see *why* it failed, not just *that* it failed.

### 2f. Get summary counts
Find the runner's totals line (e.g. "N examples, M failures" / "Tests: M failed").

Present all findings to the user before proceeding.

---

## Phase 3: Switch to the Branch

### 3a. Prepare a clean working tree
Make sure the working tree is clean before switching. If the repo documents a
procedure for restoring tracked files or local dev-file overrides (check its
`AGENTS.md`/`CLAUDE.md`, or `/git-workflow:update-main`'s Phase 2), follow it;
otherwise:
```
git stash --include-untracked   # only if you have local changes worth keeping
git fetch -p origin
```

### 3b. Switch to the PR branch — and record the remote tip
Phase 6 compares against this value and names it in the force-push lease. Read it here, before
anything rewrites the branch; if it were read at Phase 6 instead it would be compared against
itself and pass no matter who pushed:
```
git switch <BRANCH_NAME>
git pull
git rev-parse origin/<BRANCH_NAME>   # remember this: REMOTE_BASE
```
Reading it *after* the pull is deliberate: anything the pull just incorporated is already in
your branch, so leasing against the older tip would only stop a push that discards nothing.
On a branch with no remote ref yet, `git rev-parse` fails — that's the safe case (nobody can
have pushed to a branch that doesn't exist remotely); there is no `REMOTE_BASE`, so skip the
comparison in Phase 6 and push with `-u origin <BRANCH_NAME>` and no lease.

### 3c. Restore any local dev changes
If the repo's workflow has a step for re-applying local dev overrides, follow it.

---

## Phase 4: Reproduce and Fix

### 4a. Run the failing tests locally
Take the failed test files/IDs from Phase 2d and run them with the repo's test
command (check the repo's docs / `package.json` / `Makefile` / CI config for the
exact invocation). Run the failing tests together in one command where the runner
supports it — it's faster and shared setup/ordering effects surface.

### 4b. Analyze and fix
- Read the failing tests and the implementation code they exercise.
- Understand the root cause from the error messages.
- Fix the implementation (not the tests, unless the tests themselves are wrong).
- Apply the fixes directly without asking for confirmation, even when there are
  several or when one fix touches a file unrelated to the headline failure. The
  user opted in by running `/ci:fix-tests`. State the plan in a sentence or two
  as you proceed, but do not pause for approval before editing.

### 4c. Add a regression test
After fixing the root cause, add a test that would have caught this failure. Do
this without prompting — it is expected as part of every fix.
- Add it to the appropriate existing test file (mirror the source path under the
  repo's test directory).
- The test should verify the fix directly (e.g. "does not raise when X is nil",
  "returns the correct value when Y").
- Follow existing patterns in the test file.

### 4d. Verify the fix
- Re-run the originally failing tests **and** the new regression test together in
  a single command.
- Run the repo's formatter/linter on the files you touched, then verify with
  `git diff`.
- Write the commit message yourself and commit, following the repo's commit
  protocol.

---

## Phase 5: Review

### 5a. Self-review via subagent
Launch a review subagent to review the fix. The reviewer should check:
- Does the fix address the actual root cause?
- Are there other tests that might be affected?
- Does the fix follow project conventions?

### 5b. Iterate
- Apply the review's clear-cut fixes directly and re-run — the user opted in by
  running the skill (same as Phase 4b), so don't pause for approval on mechanical
  corrections.
- Surface anything that's a real judgment call (a trade-off, a risky change, a
  finding you disagree with) and let the user decide before acting on it.
- Re-run the review until it comes back clean, then report.

---

## Phase 6: Push, confirm green, take the PR out of draft

This phase isn't optional. **The push in step 1 is unconditional** — it is the only push in the
skill, so skipping it strands the 4d/5b commits on your machine whatever the PR's draft state.
Only step 3's un-draft depends on `WAS_DRAFT`; the `WAS_DRAFT: true` ending is still "CI green",
which requires steps 1 and 2. A draft is never auto-reviewed and never reported as shipped.

1. **Commit anything Phase 5b changed, then push and verify the ref actually landed.** 4d
   committed the original fix, but review iterations in 5b are uncommitted until you say so,
   and this push is new to the skill — so an uncommitted 5b fix silently never ships. Apply
   the repo's formatter and its commit protocol to that commit as 4d does; an unformatted
   commit just turns the lint check red in step 2. Don't trust the pre-push hook's output
   either (a hook's `✔ tests passed` means the hook passed, not that the push completed):
   ```
   git fetch -p origin
   git rev-parse origin/<BRANCH_NAME>        # must still equal REMOTE_BASE from 3b; if not, STOP
   git push                                  # --force-with-lease=<BRANCH_NAME>:<REMOTE_BASE> if rebased
   git rev-parse HEAD origin/<BRANCH_NAME>   # the two SHAs must match
   ```
   **If the branch was rebased, a plain `git push` is rejected as non-fast-forward** — use
   `--force-with-lease` and tell anyone sharing the branch, per the repo's rebase convention.
   (Phase 3 above only switches and pulls, but a repo whose update procedure rebases, or a
   rebase you did by hand, both land here.)

   **Comparing against `REMOTE_BASE` is the safety check, and a bare `--force-with-lease` is
   not a substitute for it:** a bare lease compares against your tracking ref, which the fetch
   on the line above just refreshed, so it is *satisfied* even when the remote has commits you
   never had — and the ref check below would then report the push as verified after destroying
   them. Naming the expected SHA makes git enforce it atomically. If the remote tip has moved
   since 3b, **stop**: someone pushed while you were working. Don't substitute
   `git log HEAD..origin/<BRANCH_NAME>` — a rebase rewrites every SHA, so that range is
   non-empty after any ordinary rebase, not just a concurrent push.

   **On a second pass through Phase 2, do not take the baseline from the remote.** After your own push,
   `REMOTE_BASE` is stale by construction — the remote tip is now the commit *you* pushed,
   which the ref check above just verified. **Set `REMOTE_BASE` to the SHA you just pushed** —
   the `HEAD` that ref check matched — rather than re-reading the remote. Re-reading adopts
   whatever is on the remote *now*, including a commit a teammate pushed while you were fixing,
   and the comparison then passes against itself and force-pushes over them.
2. **Confirm CI is green** on the pushed commit. Wait for a run to *exist* first, then watch
   it — `--watch` waits for existing checks to finish but not for them to be created, and
   with none yet `gh pr checks` fails outright rather than reporting pending:
   ```
   gh run list --branch <BRANCH_NAME> --limit 1 --json headSha,status   # until headSha == git rev-parse HEAD
   gh pr checks <NUMBER> --watch
   ```
   A bare `gh pr checks` also exits **8** on pending. Neither the "no checks reported" error
   nor exit 8 is a blocker — both just mean *not yet*.
   **If the run comes back red, go back to Phase 2** with the new failure and work it like
   the first: this push is the skill's own first push, so it can produce a red run that
   didn't exist when you started. **Bound that loop at two passes** — if the same check is
   still red after a second fix, stop and report it as a blocker rather than cycling on
   something you can't fix (flaky infra, an unrelated required job, a red base branch).
3. **Take it out of draft — only if `WAS_DRAFT` (1d) was false.** If the PR was already a
   draft when you arrived, leave it: see 1d.

   Otherwise **hand off to `/code-review:address-review-comments`**, which owns the un-draft →
   final-review → post-the-request tail. Prefer the handoff even when the PR looks clean: this
   skill has no phase for answering review findings, so un-drafting here fires an automatic
   review nobody in this skill will read — the exact state the lifecycle exists to prevent.

   Where that skill isn't available, un-draft directly (`gh pr ready <NUMBER>`) and then work
   the review yourself rather than stopping: **wait** for it to land (`gh pr view <NUMBER>
   --json reviews,comments`; `ready_for_review` triggers most automatic reviewers and they take
   a few minutes), **address** anything it raises, and only then **request** a human review by
   the repo's own mechanism. If you can't do that, say in your report that the review it fired
   is unaddressed — don't leave it implied.

   Don't try to decide the handoff by counting feedback: `gh pr view --json reviews,comments`
   returns issue-level comments and review bodies but **not** inline review threads or their
   resolve state, so it reads clean on a PR with open threads and busy on one already answered.
   The other skill gathers all three sources itself.

**Four honest endings.** Three are terminal — CI green and handed off to
`/code-review:address-review-comments` (the PR is still a draft *by design*, and that skill
owns taking it out); CI green with the PR un-drafted directly where no handoff is available;
or CI green and the PR deliberately left as you found it (`WAS_DRAFT: true`). The fourth is a
**named blocker** (a red check you didn't cause, an unmerged dependency) plus what unblocks
it — and if you drafted the PR, that ending **must** also say you left it in draft, or restore
it to ready. A blocker is not a licence to abandon a PR in a state you created.

---

## Reminders

- **Fix the cause, not the symptom** — don't loosen an assertion or delete a test to
  make CI green.
- **A PR with red CI is a draft** — 1d puts it back there so nobody reviews a branch
  you're rewriting. **This skill's own Phase 6** ends the round: it either hands the
  un-draft to `/code-review:address-review-comments` or does it directly.
- **Only un-draft what you drafted.** If the PR was already a draft when you arrived
  (`WAS_DRAFT`), leave it — it may be someone else's unfinished round. And if you stop
  early after drafting, restore it or say so; never abandon a PR in a state you created.
- Every fix ships with a regression test (Phase 4c) and follows the repo's commit,
  formatting, and staging conventions (CLAUDE.md/AGENTS.md).
