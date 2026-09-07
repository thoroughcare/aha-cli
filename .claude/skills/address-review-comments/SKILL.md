---
name: address-review-comments
description: "Work a GitHub PR's review feedback end to end: return the PR to
  draft so nobody reviews a mid-flight diff, fix each actionable comment (with a
  regression test that fails first), commit and push the fixes, reply to every
  review thread noting how it was handled and resolve the ones you addressed
  (leaving disagreed/deferred threads open with an explanation), then — once
  green — un-draft the PR, address the final automatic review it triggers, and
  post the review request. Also answers top-level review-bot summaries that have
  no inline thread. Repo- and stack-agnostic: discovers the repo's own test/lint
  commands, reviewer identity, and review-request mechanism rather than assuming
  a toolchain. TRIGGER when: the user asks to address/handle/respond to PR
  review comments, pastes a PR URL with review feedback, says \"the reviewer
  left comments\" / \"address the review items\" / \"I'd address these\", a
  reviewer (bot or human) requested changes, or types
  /code-review:address-review-comments [PR#]. SKIP when: responding to QE
  failure feedback in a ticket tracker (use the repo's failed-qe skill), or
  fixing local/CI test failures with no reviewer comments to answer (use
  /ci:fix-tests)."
---

# Address Review Comments

Turn PR review feedback into fixes that are committed, pushed, **and answered on every
thread** — never fix silently. Optional argument (PR # / URL / notes): $ARGUMENTS

The spirit of this skill: **close the loop on every comment.** A reviewer who left N
comments should, when you're done, see N threads each either resolved with a reply saying
how it was handled, or still open with a reply saying why it wasn't. No comment is left
without a response, and no fix is made without a comment pointing to it.

The order for a review round: **(1) gather → (2) triage → (3) return the PR to draft →
(4) fix → (5) commit + push → (6) reply + resolve.** Keep it in that order — the PR goes back
to draft before any of the diff changes, so nobody starts reviewing a branch that's being
rewritten, and the reply in step 6 quotes the commit SHA from step 5, which must be pushed
first for the reviewer to see it. Then **(7) un-draft → address the review that fires → post
the review request** (Phase 6).

## Phase −1: Learn this repo first

This skill is stack-agnostic, so **nothing below hardcodes a toolchain**. Before touching
code, read the repo's own conventions and use those commands throughout:

- **`AGENTS.md` / `CLAUDE.md`** at the repo root (and any nested ones covering the paths
  you'll touch) — the authority on test, format, and lint commands, commit protocol, and
  repo-specific review gates.
- **The repo's own skills under `.claude/skills/`** — a repo-local skill may own the
  ticket flow, the QE handoff, or the review-request mechanism this PR is subject to.
- **Test-file placement**: mirror the source path per the repo's documented map. Add to
  the existing mirroring test file; never create a `_<ticket>` variant.

Record the repo's test command, format/lint command, full pre-commit gate (e.g. a
`precommit`-style aggregate), and its review-request mechanism. If a documented gate is
already failing on the base branch, note it — see Phase 5 on attribution.

---

## Phase 0: Gather the review feedback

1. **Identify the PR.** From the argument (PR # / `github.com/.../pull/<n>` URL), else the
   current branch: `gh pr view --json number,url,headRefName,baseRefName,isDraft`. Note the
   number and draft state; the `gh api` calls below use `{owner}`/`{repo}` placeholders.
2. **Identify the reviewer — read it, don't assume it.** Bot logins differ per repo and are
   frequently misdocumented (a workflow comment claiming one App name while the API reports
   another). Get it from the PR itself:
   ```bash
   gh pr view <pr> --json reviews --jq '[.reviews[].author.login] | unique'
   gh api repos/{owner}/{repo}/pulls/<pr>/comments --jq '[.[].user.login] | unique'
   ```
   You need the real login twice below: to recognise a bot summary review as the feedback to
   work from (step 3), and to tell bot findings from human ones when deciding whether the PR
   is ready to un-draft (Phase 6.1) — an unanswered *bot* finding blocks, a human
   disagreement doesn't. Assume a login and both calls quietly key off a name nobody posts
   under.
3. **Make sure there's a review to work from.** If the PR already has unaddressed feedback
   (inline threads, or a recent bot summary review), continue. Otherwise trigger one — and
   **don't passively wait on a draft.** Many repos gate their automatic reviewer on
   `draft == false`, so a draft is never auto-reviewed; check the workflow (commonly
   `.github/workflows/claude.yml`) for the gate. Where an interactive reviewer exists:
   ```bash
   gh pr comment <pr> --body "@claude review"
   ```
   Then poll until it lands (allow a few minutes):
   ```bash
   gh pr view <pr> --json comments,reviews
   ```
4. **Fetch every source of feedback** — a reviewer uses more than one:
   - **Inline review comments (threads):**
     `gh api repos/{owner}/{repo}/pulls/<pr>/comments --paginate` — each comment's `id`,
     `path`, `line`, `body`, `user.login`, `in_reply_to_id`.
   - **Review summaries / verdicts:** `gh pr view <pr> --json reviews` — the top-level body
     of each review.
   - **Issue-level comments:** `gh api repos/{owner}/{repo}/issues/<pr>/comments --paginate`
     — where a bot posts its full report, and where a human may say "I'd address these".
   - **Thread resolve-state + node ids:** the GraphQL query in Phase 4.
5. **Build the work-list.** Only threads that are **unresolved** and **not already
   answered** (skip any whose latest reply is your own "Fixed in `<sha>`…"). A single
   summary review can bundle several findings — split it into one work-item per finding.
   Note which findings live only in a summary (no thread to resolve) vs. an inline thread
   (has a thread to reply-to **and** resolve).

---

## Phase 1: Triage each item

For each work-item, record exactly one disposition:

- **Fix** — the comment is right; change the code (Phase 2).
- **Already correct / N/A** — the concern doesn't apply; no code change, but still **reply**
  explaining why (Phase 4) and resolve.
- **Disagree** — you believe the current code is right; no change, **reply** with the
  reasoning, and **leave the thread open** for the reviewer to weigh in.
- **Defer** — valid but out of scope; file/link a follow-up ticket, **reply** with the link,
  and **leave open**.

Only **Fix** and **Already correct/N-A** get resolved. Never resolve a thread you disagreed
with or deferred — that hides an unsettled point.

Being right matters more than being agreeable: a reviewer (especially an automated one) can
be wrong, and "Disagree" with reasoning is a better outcome than a change that makes the code
worse. Equally, verify before disagreeing — reproduce the claim first.

---

## Phase 1b: Return the PR to draft

If triage produced at least one **Fix**, the diff is about to change — so the PR is not ready
for review, whatever its current state says. Put it back in draft **before you touch the
code**:

```bash
gh pr ready --undo <pr>     # no-op if it's already a draft
```

A PR marked ready-for-review is an invitation. Leaving it there while you rewrite the diff
lets a reviewer start on code that's already being replaced, and their comments land on lines
that no longer exist — wasted reviewer time and a review round that's stale before it's read.
Draft is the honest state for a branch that's mid-flight.

- **Skip it** when the PR is already a draft, or when triage produced no **Fix** items — a
  round of pure Disagree/Defer/Already-correct replies changes no code, so reply and leave the
  state alone.
- **Say so in your report.** A silent state change is invisible to whoever is watching the PR.
- **You now own the un-draft.** A draft is never reported as shipped; Phase 6 takes it back out
  once the round is green.
- **This applies to a Phase 6 loop-back too.** If the review that fires on un-draft turns up a
  real **Fix**, the diff is mid-flight again — draft it again for that round. The gate is
  "does code change", so a loop-back that only needs replies leaves the PR ready.

**If you stop anywhere before Phase 6 and the PR is a draft, say so in your report** — a
pre-push hook you won't bypass (Phase 3), a pre-existing red gate you were told to report
rather than silently fix (Phase 5), a **Fix** you can't land. Restore it to ready if the round
genuinely isn't happening; otherwise name the draft and the blocker. This holds **whoever
drafted it**: `/ci:fix-tests` and a repo's `failed-qe` both draft a PR and hand off here, so
"Phase 1b didn't draft it" is not evidence that someone else owns it. Going quiet on a draft is
how a change disappears — it is invisible to reviewers *and* unreported as blocked.

---

## Phase 2: Fix (with a test that fails first)

For each **Fix** item:

1. Make the change at the right altitude, following the repo's documented architecture.
2. **Add or update the covering test, and prove it catches the issue** — run it with the fix
   reverted and confirm it **fails** at the layer the reviewer cares about, then re-apply and
   confirm it passes. Assert the observable outcome (persisted state, user-visible message,
   emitted event, response), not intermediate internal state.
3. Add tests to the existing mirroring test file per Phase −1.
4. Run all affected tests in **one** command, then the repo's format/lint commands.
5. **Apply the repo's own review gates** — the ones `AGENTS.md`/`CLAUDE.md` say are
   PR-blocking. These differ per repo; common families worth checking for: test-id / QA
   selector contracts, observability (an event registered *and* asserted), feature-flag
   gating, multi-tenancy scoping with an isolation test, design-token usage, and an
   accessibility baseline. Grep the repo's conventions rather than guessing.
6. **Docs:** if the fix changes behavior a spec/plan/story doc describes, update it in the
   same commit per the repo's doc-sync convention. Pure CI/tooling/refactor fixes are
   usually exempt.

---

## Phase 3: Commit and push

Follow the repo's **commit protocol**: run its formatter after tests pass, verify with
`git diff`, stage **only** the files you changed (`git add <path>` — never `git add -A` /
`git add .`, which sweeps in untracked local scratch and any dev-override files the repo
tells you not to commit), then write the commit message yourself and commit directly.

- Group the round into one commit, or a few logically-separate ones (e.g. one per concern),
  so each reply can point at a specific SHA.
- **Push before replying** — the reply quotes the SHA, and the reviewer can only see it once
  it's on the remote. If a rebase rewrote history, use `git push --force-with-lease`, and
  pause for the user to confirm the force-push first.
- Capture each short SHA (`git rev-parse --short HEAD`).
- **If a pre-push hook blocks the push**, diagnose before bypassing: reproduce the hook's
  checks standalone. If the hook is broken independently of your diff (verify by running it
  on the base branch), run its checks manually, then push with `--no-verify` and **say so in
  your report**. Never bypass a hook that's failing because of your change.

---

## Phase 4: Reply to every thread, then resolve the ones you addressed

For **each inline thread** in the work-list:

1. **Reply** — never fix silently:
   ```bash
   gh api -X POST repos/{owner}/{repo}/pulls/<pr>/comments/<comment_id>/replies \
     -f body='Fixed in `<sha>`: <one line on what changed / why>.'
   ```
   (`<comment_id>` = the thread's first-comment `id` from Phase 0.) If your fix differs from
   what was suggested, say so and why — and if the comment's diagnosis was partly wrong,
   correct it plainly while crediting the finding.
2. **Resolve** — **Fix** / **Already-correct** only. REST has no resolve endpoint, so use
   GraphQL. Fetch thread node ids, matching the thread whose first comment `databaseId`
   equals the `<comment_id>` you replied to:
   ```bash
   gh api graphql --paginate -f query='
     query($owner:String!,$repo:String!,$pr:Int!,$endCursor:String){
       repository(owner:$owner,name:$repo){ pullRequest(number:$pr){
         reviewThreads(first:100, after:$endCursor){
           pageInfo{ hasNextPage endCursor }
           nodes{
             id isResolved
             comments(first:1){ nodes{ databaseId author{login} path } }
           } } } } }' -F owner={owner} -F repo={repo} -F pr=<pr>
   ```
   `first:100` is GitHub's per-page cap, so **paginate** — a busy PR exceeds it and the tail
   is dropped with no error, leaving threads unanswered on a skill whose whole point is
   answering all of them. `gh api graphql --paginate` walks the cursor for you, but only if
   the query declares `$endCursor` and selects `pageInfo{ hasNextPage endCursor }` as above.
   then resolve the matched thread id:
   ```bash
   gh api graphql -f query='mutation($id:ID!){
     resolveReviewThread(input:{threadId:$id}){ thread{ isResolved } } }' -F id=<threadId>
   ```
   For **Disagree** / **Defer**: reply, but **do not** resolve.

For findings from a **top-level summary review** (no inline thread), post **one**
consolidated issue-level comment covering what was addressed, with the SHA(s):
```bash
gh api -X POST repos/{owner}/{repo}/issues/<pr>/comments -f body='<summary of fixes + SHAs>'
```

---

## Phase 5: Close the loop

- Re-run the Phase 4 thread query — **with the same `--paginate`**, or the check passes
  against a truncated list: every **Fix**/**Already-correct** thread shows
  `isResolved: true`, and every open thread has a reply explaining why it's open.
- Re-run the repo's full pre-commit gate so the round ends green.
- **Attribute failures before owning them.** An aggregate gate can be red on the base branch
  for unrelated reasons (a dependency advisory, a stale-state false positive). Re-check on a
  clean build and compare against the base branch before treating a failure as yours — and
  don't silently "fix" a pre-existing failure inside a review round; report it.
- Report per comment: disposition + SHA (or why left open), plus push status.

---

## QE / test-ownership dispositions

Some repos label a PR to record who owns its verification (e.g. a "not testable by QE" or
"QE deferred" label). Where that convention exists:

**Never set such a disposition yourself.** It takes work off another team's plate, so it is
agreed **jointly** with them — not assigned by the PR author, and not inferred from a review
comment. Removing something from QA scope unilaterally is how coverage silently disappears.

State the author-side reasoning in the PR's QE/testing section — what surface exists, what an
end-to-end test could exercise, where the in-repo coverage sits — then surface it for that
conversation and leave the PR **unlabeled** until agreement exists. Check the repo's own
convention for any narrow, mechanical carve-out (some allow self-labelling a strictly
docs-only diff); treat anything touching code as needing the agreement.

Regardless of disposition, **always internalize coverage in-repo** up to the end-to-end
boundary — deferring the QA layer never waives the unit/integration safety net.

---

## Phase 6: Un-draft → address the review that fires → post the request

Many repos open PRs as drafts so bot feedback is cleaned up before a human is pinged, and
gate the automatic reviewer on `draft == false`. Where that's the case, un-drafting is the
**last** step, not the first — and it triggers a fresh review that must itself be cleared
before anyone is pinged.

1. **Confirm it's ready:** every Fix/Already-correct thread resolved; every open thread has
   an explanatory reply (un-drafting with an unanswered *bot* finding is not OK; carrying a
   disagreement you want a human to weigh in on is fine); CI green. If a rebase rewrote
   history, get the force-push in first so the PR shows the rebased commits — and if Phase 1b
   was skipped (no **Fix** items) the PR is still **ready-for-review**, so draft it for that
   force-push and un-draft in step 2. Rewriting the branch under a reviewer is the thing this
   skill exists to prevent; it doesn't stop being true on the no-Fix path.

   **If this gate doesn't open** — an unanswered bot finding you can't resolve, or a red check
   Phase 5 told you to report rather than silently fix — apply the stop rule in Phase 1b: the
   PR is a draft, so say so in your report, whoever drafted it. Reporting a blocker is
   correct; going quiet on a draft is not.
2. **Un-draft:** `gh pr ready <pr>`. Check what this fires **in the repo you're actually in** —
   commonly a ticket state transition and a fresh automatic review. Note whether it also sends
   the human review notification, or whether that's a separate deliberate step (step 4).
   `ls .github/workflows/ | grep -i 'linear\|ready_for_review'` answers it in one line.

   **Don't assume the ticket moved.** Plenty of repos have no such automation, in which case
   nothing advances the ticket and you must move it yourself. A tracker's own VCS integration
   may separately have moved it to an in-progress state when it noticed the branch, which makes
   a stalled ticket look managed. And when you do set the state by hand, **verify the write** —
   state names differ per team and a wrong one often doesn't error, it just lands somewhere
   else, so assert the returned status is the one you asked for.
3. **Address the review that lands.** Poll `gh pr view <pr> --json reviews,comments`. Ordinary
   pushes often don't re-trigger the reviewer (only `opened`/`ready_for_review` do), so this
   is typically the one final automatic pass. Loop back through Phases 0–5 for any findings — start at Phase 0, not Phase 1: it is the only place inline threads and their resolve state are fetched, and `gh pr view --json reviews,comments` above returns neither.

   **If that loop-back re-drafted the PR (Phase 1b), repeat step 2 before step 4** — otherwise
   the request in step 4 goes out on a draft, which most repos will hold rather than deliver.
   The loop converges when a review lands no **Fix** items.

   **If it hasn't converged after two rounds, stop — but un-draft before you do.** Step 1's
   gate says not to un-draft with an unanswered bot finding, and unconverged rounds are
   exactly that; this is the deliberate exception. That gate governs declaring a round
   *finished*, and this is an escalation, not a finish. Leaving it drafted makes the change
   invisible to reviewers *and* unreported as blocked, which is strictly worse. Take it out of
   draft, then surface the unconverged findings to the user as a named blocker.
4. **Post the review request** using the repo's own mechanism (a script under `bin/`, a
   webhook, or a manual channel message — Phase −1 identified it), only once the review is
   addressed and CI is still green. Fail soft: if the webhook/secret isn't configured, fall
   back to the repo's manual message.

**Fast path.** Where the repo defines one, a PR whose diff is plainly non-testable
(markdown/docs/agent-config only, nothing under source or test trees) collapses steps 2–4:
un-draft and post as soon as CI is green, without holding for a review that has nothing to
say. Watch for **skipped jobs counting as green** — repos commonly gate heavy jobs behind a
"detect non-doc changes" step, so a docs-only PR shows them `skipping`, which is a pass.

**Decide from the PR's current state, not from who drafted it.** `gh pr view <pr> --json isDraft`
is the only input that matters, because this skill is the named un-draft owner for several
others: `/ci:fix-tests` and a repo's `failed-qe` both draft a PR and hand the un-draft here. A
draft you didn't create is the normal case, not an anomaly — and it has usually had commits
pushed to it by that upstream skill, so don't assume its last review covers the current diff.

- **`isDraft: true`** — un-draft it (step 2), whoever drafted it and for whatever reason.
  Expect that to fire a fresh review (`ready_for_review` triggers most automatic reviewers);
  clear it as in step 3 before posting the request.
- **`isDraft: false`** — there's nothing to un-draft: clear any outstanding review, confirm CI,
  then post the request.

---

## Reminders

- **Every comment gets a response; every fix gets a comment.** Done means: no unanswered
  thread, no silent fix.
- **A PR whose diff is being rewritten is a draft.** Return it to draft before changing code
  (Phase 1b) and take it back out when the round is green (Phase 6) — and never leave it
  ready-for-review while you push fixes.
- **Never go quiet on a draft.** Ending with the PR drafted is allowed only when you say so
  and name the blocker; ending with it drafted and unmentioned is not.
- **Order:** gather → triage → draft → fix → commit + push → reply + resolve → un-draft →
  address the final review → post the request.
- **Only resolve what you actually addressed** — disagreements/deferrals stay open, answered.
- **Tests must fail without the fix.**
- **Read the reviewer's login and the repo's commands; don't assume either.**
- **Never self-assign a QA/QE disposition** — it's a joint call.
- **Paginate every listing** — comments, issue comments, and review threads. A silently
  truncated list is an unanswered thread.
- Not for QE failure feedback, or comment-less CI failures (that's `/ci:fix-tests`).
