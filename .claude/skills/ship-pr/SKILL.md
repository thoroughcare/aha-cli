---
name: ship-pr
description: "Carry committed work the rest of the way to a review-requested PR:
  size the diff and split it along a layer seam if it's too large to review,
  push and verify the ref actually landed, open the PR (draft-first where the
  repo works that way), trigger the review, then hand the review round to
  /address-review-comments and land every touched repo with /update-main. Repo-
  and stack-agnostic — discovers the repo's default branch, test gate, PR
  conventions and review-request mechanism rather than assuming a toolchain.
  TRIGGER when: a change is committed and green and needs to reach a reviewer —
  \"push this and open a PR\", \"ship it\", \"open the PR\", \"request review
  for this\", \"is this PR too big / should I split it\", or /ship-pr. ALSO run
  it unprompted once work is committed and tests pass, whatever brought the
  change in (a ticket, a \"fix this\", a review comment, a follow-up). SKIP
  when: nothing is committed yet, a local test is genuinely failing (fix that
  first), or the PR is already open and the task is working its review feedback
  (use /address-review-comments)."
---

# Ship PR

Take committed, green work to a **review-requested PR**. Optional argument (PR # / ticket
/ notes): $ARGUMENTS

**Finished work is a review-requested PR, not a clean working tree.** A change that is
committed and passing but sitting on a local branch has not shipped, and neither has a PR
left in draft. This skill is the sequence that closes that gap, and the checks that stop
you from claiming it closed when it isn't.

**What authorizes shipping is the work, not the door you came in through.** A ticket-start
skill authorizes it — and so does "fix this", a review comment, a follow-up someone asked
for, and a correction to something just merged. Scoping the authorization to a couple of
entry points is what strands ad-hoc work one step short of shipped: the change is committed
and green, and the flow stops to ask for a permission that "fix this" already implied.

**Offering the remaining steps back as a choice is the failure mode, not politeness.** "I
haven't opened the PR — say the word and I will" reads as progress while leaving the work
unshipped. If you have committed, green work and no instruction to hold it, run the
sequence. Stop only for a **real local test failure** or a genuinely destructive git
operation (force push over someone else's commits, `reset --hard` on a shared branch).

---

## Phase −1: Learn this repo first

Nothing below hardcodes a toolchain. Read the repo's own conventions and use those:

- **`AGENTS.md` / `CLAUDE.md`** at the repo root (plus any nested ones covering the paths
  you touched) — the authority on the test/format/lint commands, the full pre-commit gate,
  branch and PR naming, commit protocol, and any review or testing-disposition gates.
- **`.github/workflows/`** — what CI actually gates, whether the automatic reviewer is
  gated on `draft == false`, and whether opening a non-draft PR moves a tracker ticket on
  its own (`ls .github/workflows/ | grep -i 'linear\|jira\|ready_for_review'`).
- **The default branch** — `git remote show origin | sed -n 's/.*HEAD branch: //p'`, not an
  assumed `main`. Sibling repos frequently differ (`main` in one, `master` in another).
- **The review-request mechanism** — a script under `bin/`, a webhook, a repo-local
  request-review skill, or a manual channel message.

Record all of it before you push; several steps below branch on it.

---

## Phase 0: Size the diff — split before you open, not after a reviewer asks

**A PR that changes more than roughly 2,000 lines is too large to review. Split it before
opening it.** Check first, because after the PR is open a split costs a round trip:

```bash
git diff --stat <default-branch>...HEAD | tail -1
```

The threshold is **a prompt to look for the seam, not an exact bound**: 2,100 lines that
genuinely can't be cut is fine if you say why in the PR body; 1,400 lines with an obvious
seam should still be split.

**Split along the layer seam, not by file count or "part 1 / part 2".** Each PR has to
stand on its own as a reviewable unit with a claim a reviewer can check. "The Risk Levels
data layer, read-only, writes delegate to the legacy app" is such a claim. "The first 700
lines of a LiveView" isn't. Where the seam falls depends on the stack and the work — read
the repo's agent docs for a documented seam before inventing one. Common shapes:

- **A domain half and a web half.** Everything under the domain/model tree plus its tests
  — the context or service objects, the schema, the policy, the write delegation, the
  telemetry — has **no reachable web surface**, so it reviews as a self-contained data
  layer. The routes, the controller or LiveView, the templates and their tests follow in a
  second PR **stacked on the first**.
- **A background worker and the code that enqueues it.**
- **A migration and the code that reads the new column.**
- **A shared component and its first consumer.**

Stack the second PR on the first so it shows only its own files; GitHub retargets it to the
default branch when the first merges:

```bash
gh pr create --base <first-branch> --draft
```

**The test of a split is that each half compiles and passes its own tests standalone.**
Verify that on each branch rather than assuming it — a split that only builds when both
halves are applied isn't a split, it's a diff cut in two.

Two more things that make a large diff read smaller, worth doing at any size:

- **A generated or mechanical change** — a rename, a formatter pass, a regenerated
  lockfile — belongs in **its own commit or PR**, so the substantive diff isn't buried in
  it.
- **A large test file is still large.** Splitting the code but leaving a 1,000-line test
  file with one half doesn't help much; consider whether some cases belong with the layer
  they actually exercise.

**If an oversized PR is already open, it's still worth splitting.** Open the two
replacements and close the original with a comment linking them, carrying over any review
that already landed so the discussion isn't lost.

---

## Phase 1: Push — and verify the ref actually landed

```bash
git push -u origin <branch>
```

**A push has landed only when the remote ref says so.** Every later check reads the PR,
which is downstream of the commit actually arriving, so verify the push itself first:

```bash
git fetch -p origin
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/<branch>)" ] || echo "NOT LANDED"
```

**The trap is the pre-push hook.** A hook that runs the test suite prints its own progress
(`✔ lint`, `✔ tests passed`), and those lines mean **the hook passed** — not that the push
completed. `git push` can still exit non-zero *after* them, and a `141`/SIGPIPE from a piped
or backgrounded push is silent unless you check `$?`. A push that died there leaves the
reviewer looking at an older commit while the local branch looks finished, and any review
reply quoting a SHA is worthless because the commit isn't on the remote. So check the exit
code **and** compare the refs before saying a branch is pushed.

---

## Phase 2: Open the PR

Open it against the repo's default branch, linking the tracker ticket if there is one, and
following the repo's PR title/body conventions from Phase −1.

**Draft-first where the repo works that way** (check whether its automatic reviewer is
gated on `draft == false`):

```bash
gh pr create --draft
```

A draft keeps the human review request — and the tracker's *Review* state — held until CI
is green and the bot feedback is cleaned up, so a human is engaged on a reviewed, green PR
rather than a raw one. In a repo with no draft convention, open it ready and take its
`opened` review as the first round.

---

## Phase 3: Trigger the review — a draft is NOT auto-reviewed

Automatic reviewers are commonly gated `draft == false`, so on a draft the review **will
never fire on its own**. Don't passively wait for it. Where an interactive reviewer exists,
ask for it explicitly:

```bash
gh pr comment <pr> --body "@claude review"
```

Otherwise request a human reviewer. Check the workflow file rather than assuming either way
— "waiting for the review" on a draft that nothing will review is the most common way this
sequence stalls.

The readiness check before this step: the relevant tests green locally, then CI green on the
pushed branch. **That check is the checkpoint** — a genuine local failure is where you stop
and surface the problem, not a yes/no "shall I continue?" prompt.

---

## Phase 4: Hand the review round off

Once a review lands, the work of reading it, fixing each item, replying to every thread,
un-drafting, and posting the review request belongs to
**`/address-review-comments`** — it owns that loop (including the un-draft →
final-review → post-the-request tail, and the non-testable fast path). Invoke it rather
than re-deriving those steps here.

Two rules from that loop matter enough to restate, because they are how a PR gets
mis-reported as shipped:

- **A draft PR is never reported as shipped.** Two honest endings exist, and neither is a
  status line that says "draft": CI is green and the review is addressed → **un-draft it**;
  or something genuinely blocks it (a red check you didn't cause, an unmerged dependency PR,
  an unanswered question) → **say so explicitly**, name the blocker, and name what unblocks
  it.
- **Never self-assign a QE/testing disposition** where the repo has one. It takes work off
  another team's plate, so it's a joint call — see that skill's QE section for the narrow,
  location-based carve-outs some repos allow.

---

## Phase 5: Land the working tree

The sequence does not end at the un-draft. Run **`/update-main`**, which owns
the rule (see its "Landing every repo after a ship"): it returns **every repo the change
touched** to its default branch — not just the one you're standing in, which matters for
work mirrored across sibling repos — and prunes local branches whose upstream is `[gone]`.
The branch you just shipped survives, since its upstream exists while the PR is open.

Don't raise it as a question: being told *"switch both to master"* means this step was
skipped.

**Not before the un-draft.** Landing while the review round is still committing to the
ticket branch strands the rest of the sequence on a stale base.

---

## Verify the terminal state before you claim it

This applies to **every** ending, including a fast path: **the summary you write is not
evidence.** Read the state back, and report what you actually saw.

```bash
gh pr view <pr> --json isDraft,state    # draft is not a terminal state
gh pr checks <pr>                       # green, or name the red check
gh pr view <pr> --json reviews --jq '.reviews[] | "\(.author.login) \(.state)"'
git branch --show-current               # in every repo the change touched
```

**The working tree is part of the terminal state.** If `git branch --show-current` still
reports the ticket branch in any repo the change touched, Phase 5 hasn't run — the same
one-step-short failure, one step later, and the one the PR-state checks above sail straight
past, because they all pass while you're still standing on the branch.

**Don't reach for `reviewDecision` to answer "was this reviewed."** A comment-only reviewer
(many repos instruct theirs not to approve or request changes) leaves that field `null` /
`REVIEW_REQUIRED` however many reviews landed. Count the reviews themselves, as above — and
confirm no thread was left hanging:

```bash
gh api graphql -f query='
  query($owner: String!, $repo: String!, $pr: Int!) {
    repository(owner: $owner, name: $repo) {
      pullRequest(number: $pr) {
        reviewThreads(first: 100) { nodes { isResolved path } }
      }
    }
  }' -F owner=<owner> -F repo=<repo> -F pr=<pr> \
  --jq '[.data.repository.pullRequest.reviewThreads.nodes[]
         | select(.isResolved | not)] | length'
```

**"Waiting on X" is a claim about state too — check X once before you stop.** The rule above
catches overclaiming; this catches the opposite failure, going idle while work sits ready. A
background waiter can silently never fire (a `--jq` filter that can't match, a condition that
was already true, a poll loop watching the wrong field), and its silence is
indistinguishable from "nothing has happened yet". Before ending a turn on *waiting*, query
the thing directly — the reviews, the checks, the refs — so what you report is an
observation. Prefer polling that names every terminal state over one that greps for the
happy path.

Two mechanics make that reliable, and a waiter missing either can strand a session for hours:

- **Bound the loop, and let it say "never".** Give every wait a maximum iteration count and,
  on timeout, exit **non-zero naming what it was still waiting for** — an unbounded loop whose
  predicate can never be satisfied never exits, so no notification fires and nothing re-invokes
  you. Keep "**no rows matched**" distinct from "**still running**": a filter matching zero rows
  is not a job in progress, and folding the two together is how a loop waits forever for
  something that will never exist.
- **Pair it with an independent deadline timer.** A bound only helps while the waiter is alive
  and its predicate is the only thing wrong; it does nothing when the waiter dies, is killed, or
  exits on a false positive. A separate command that does nothing but expire —
  `sleep <deadline>; echo "deadline reached: <what you were waiting on>"` — fires regardless,
  because it does not depend on the waiter being correct or even alive. It is not a poll: it
  re-checks nothing and expires once.

**Order the two deadlines, and count sleeps rather than iterations.** The timer must expire
*after* the waiter's own bound, or it fires first and you lose the waiter's more specific
message. And a loop of N iterations sleeps N-1 times, so `for i in $(seq 1 25) … sleep 60`
times out at ~24 minutes, not 25 — set the bound off the work's *slow* case, not its typical
one. A CI run that usually takes 20 minutes but varies with matrix and dialyzer will
sometimes take 30, and a waiter that gives up at 24 reports a confident false timeout on a
run that was about to pass.

Use as many waiters and timers as the situation warrants. What matters is that something always
wakes the session, not how few things are watching.

**Anything you cannot read back is reported as unconfirmed.** A review-request webhook is
the usual case: its `{"ok":true"}` and the notifier script's success line look identical
whether the message rendered or was dropped for a bad payload. Confirm by eye in the channel,
or report it as **posted-but-unconfirmed** — never as done.

---

## Reminders

- **Split before opening**, along a layer seam, each half green standalone (Phase 0).
- **Verify the push landed by comparing refs** — hook output is not push success (Phase 1).
- **A draft is not auto-reviewed**; trigger it or it never comes (Phase 3).
- **Hand the review round to `/address-review-comments`**; don't re-derive it.
- **End at the landing, not the un-draft** — `/update-main`, every touched repo.
- **Read the terminal state back before claiming it**; a draft is never "shipped".
- Only pause for a real local test failure or a genuinely destructive git operation.
