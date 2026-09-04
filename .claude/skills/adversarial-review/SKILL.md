---
name: adversarial-review
description: "Review a change to convergence instead of once: dispatch a fresh
  review subagent, fix what it finds above nit severity, then dispatch another —
  round after round, each round carrying forward what was already fixed and
  dismissed, until a full round surfaces nothing above a nit. Rotates the review
  lens per round so a later round isn't a rerun of the first, caps rounds so a
  reviewer/fixer standoff can't loop forever, and reports a round-by-round
  ledger with an explicit CONVERGED / CAPPED / ESCALATED verdict. Repo- and
  stack-agnostic — discovers the repo's own test and lint commands rather than
  assuming a toolchain. TRIGGER when: the user asks to review something
  adversarially or to convergence, \"review this until it's clean\", \"keep
  reviewing until only nits are left\", \"hammer on this diff\", wants
  repeated/looping review passes, or types /code-review:adversarial-review
  [target]. SKIP when: one review pass is what was asked for (use the repo's
  ordinary review skill or /code-review), or the findings already exist as PR
  review comments to work through (use /code-review:address-review-comments).
  Args: [target] — what to review: a PR number/URL, a branch, a path, or nothing
  for the current diff. Optionally an effort level (low/medium/high/max) and a
  round cap."
---

# Adversarial Review

Review a change until the reviewer runs out of things to say. Optional argument
(target / effort / round cap): $ARGUMENTS

A single review pass reports whatever it happened to notice. That is a weak signal,
because a pass that finds three bugs tells you nothing about the fourth. The signal
worth having is **the absence of findings** — a competent reviewer, looking at the
change fresh, with the earlier findings already fixed, finding nothing left worth
raising. That takes more than one pass, and it is what this skill produces.

The shape is a loop:

> **dispatch a fresh reviewer → fix everything above nit → dispatch another fresh
> reviewer → …** until a full round comes back with nothing above a nit.

Two properties make it work, and both are easy to lose:

- **The reviewer is never the fixer.** Each round dispatches a **new subagent with no
  memory of the previous rounds' reasoning**. Whoever just wrote a fix is the worst
  possible judge of it — they know what they intended, so they read the intent rather
  than the code. A fresh reviewer reads only what is there.
- **Rounds don't repeat themselves.** Each round gets the *ledger* (what was already
  found, fixed, and dismissed, and why) and a **different lens**, so round 3 is looking
  somewhere round 1 didn't rather than re-reporting round 1's list.

## Phase −1: Learn this repo first

Nothing below hardcodes a toolchain. Before dispatching anything, read the repo's own
conventions and use those commands for the rest of the run:

- **`AGENTS.md` / `CLAUDE.md`** at the repo root, plus any nested ones covering the paths
  in the diff — the authority on the test command, the format/lint command, the full
  pre-commit gate, the commit protocol, and any repo-specific review gates (multi-tenancy
  scoping, PHI handling, telemetry, N+1 coverage, `data-testid` requirements).
- **The repo's own skills under `.claude/skills/`** — a repo-local skill may own the
  review checklist, the test-file mirror map, or a domain gate this diff is subject to.
  A repo-local rule outranks a generic instinct.

Record the test command, the format/lint command, the aggregate pre-commit gate, and the
repo's review gates. **The review gates go into every round's reviewer prompt** — a
reviewer that hasn't been told the repo requires a cross-tenant test cannot report its
absence.

If a documented gate is already red on the base branch, note it now. A pre-existing
failure is not a finding against this diff, and mistaking it for one wastes a round.

---

## Phase 0: Establish the target and the baseline

1. **Resolve what is under review** from `$ARGUMENTS`:
   - a **PR number / URL** → `gh pr diff <pr>`, and note the head branch
   - a **branch** → its diff against the repo's default branch
   - a **path** → the changes under it, or the file set itself if there is no diff
   - **nothing** → the current working diff (`git diff` + `git diff --cached`), falling
     back to the branch's diff against the default branch if the tree is clean
2. **Read the diff yourself before dispatching.** You are about to brief several
   subagents on it; a briefing written without reading it produces reviewers looking in
   the wrong place. Note the surfaces touched (context/model, web/controller/view,
   migration, worker, config) — these drive the lens rotation in Phase 1.
3. **Establish a green baseline.** Run the repo's test gate now and record the result. A
   round that "found a failing test" needs to know whether the diff caused it.
4. **Pick the round cap.** Default **5**. `$ARGUMENTS` may override it. The cap exists to
   bound a standoff, not to be hit — a run that hits it reports `CAPPED`, which is not a
   pass.
5. **Open the ledger.** One row per finding, for the whole run:

   | # | Round | Severity | File:line | Finding | Disposition | Evidence |
   |---|-------|----------|-----------|---------|-------------|----------|

   The ledger is the skill's memory. Subagents are fresh each round; **the ledger is
   what carries across them**, and it is the final report.

---

## Phase 1: Run a round

Repeat this phase until Phase 2 says stop.

### 1a. Dispatch a fresh reviewer

Dispatch **one review subagent per round**, in the background, with no context from
previous rounds beyond the ledger you hand it explicitly.

Where the environment has a review skill that already encodes the repo's standards
(`/code-review` at a given effort, or a repo-local review skill found in Phase −1),
have the subagent run **that** rather than reinventing a checklist — it will apply
conventions this skill doesn't know about. Otherwise brief it directly.

The prompt carries, every round:

- **The target** — the diff (or how to obtain it), and the base to compare against.
- **This round's lens** (1b) — where to look, stated as the round's *primary* focus and
  not as a restriction. A reviewer that finds a data-loss bug while assigned the
  "tests and coverage" lens reports it.
- **The repo's review gates** from Phase −1, verbatim. These are findings the reviewer
  cannot otherwise know to look for.
- **The ledger so far** — every prior finding with its disposition, and for each
  dismissal, **the reason it was dismissed**.
- **The severity ladder** (1c), and an instruction to assign a severity to every
  finding. Severity is the **reviewer's** call.
- **The standing instruction to verify before reporting.** A finding needs a concrete
  failure scenario: inputs or state → the wrong output, crash, leak, or missed gate. A
  finding that can't be stated that way is speculation, and speculation costs a round.

**On re-raising a dismissed finding:** the reviewer may — and should — re-raise
something the ledger records as dismissed, **but only by showing the dismissal reason is
wrong**, and it must say so explicitly. Suppressing that outright would let a bad
dismissal end the run, which is the failure mode this whole skill exists to prevent. A
re-raise that just restates the original finding without engaging the reason is noise;
say so in the ledger and move on.

### 1b. Rotate the lens

Assign each round a different primary lens, ordered so the expensive-to-miss things come
first. Adapt the list to what the diff actually touches — a migration-only diff has no
UI lens — and log which lens each round used:

| Round | Primary lens |
|---|---|
| 1 | **Correctness** — logic errors, wrong branches, off-by-one, nil/empty/boundary handling, error paths that swallow failure, incorrect state transitions |
| 2 | **Data and security** — tenant/permission scoping, authorization gates, injection through unchecked input, sensitive data in logs/metrics/responses, transaction and rollback correctness, data loss on the failure path |
| 3 | **Tests** — what the diff can break without a test failing; missing equivalence classes, boundaries, guard clauses reachable only by a forged event, and whichever coverage the repo mandates (query-count, cross-tenant, telemetry) |
| 4 | **Interfaces and contracts** — callers this change breaks, the shape crossing a process/network/repo boundary, migration/rollback safety, backward compatibility |
| 5 | **Simplification and reuse** — duplicated logic, an abstraction the repo already has, code at the wrong altitude, dead paths |

Past the cap-length list, rotate back to **Correctness** with an explicit instruction to
look where earlier rounds didn't.

### 1c. Severity ladder

The exit condition is a severity threshold, so the ladder has to mean something. The
reviewer assigns one per finding:

| Severity | Definition | Effect |
|---|---|---|
| **Blocker** | Wrong behaviour, data loss or corruption, a security/tenancy leak, or a crash on a reachable path. | Must fix. Blocks convergence. |
| **Major** | Real defect on a less-likely path; a missing test for behaviour the repo requires covered; a contract broken for a caller. | Must fix. Blocks convergence. |
| **Minor** | Genuine but low-impact — a narrow edge case, an unclear error message, a small inefficiency with no scale behind it. | Must fix or explicitly dismiss with a reason. Blocks convergence. |
| **Nit** | Style, naming, comment wording, formatting, subjective preference. No behavioural consequence whatsoever. | Does **not** block. Fix if trivial, otherwise note and move on. |

**The fixer never re-grades a finding.** Believing a Minor is really a Nit is a
*disagreement*, and it goes in the ledger as one — dismissed-with-reason, which still
blocks the round and hands the next reviewer the reason to attack. Letting the fixer
downgrade its way to an empty round is exactly how this loop gets faked, and the whole
value of the run is that the empty round is real.

### 1d. Fix what the round found

Work the findings in severity order, and fix them **yourself** — do not hand fixing back
to the reviewer.

- **A fix for a behavioural finding ships a test that fails first.** Write the test,
  watch it fail against the current code, then fix until it passes. A regression test
  that never failed is asserting nothing.
- **Follow the repo's test-file placement** from Phase −1 — add to the mirroring test
  file, never a new variant beside it.
- **Dismissing is allowed; dismissing silently is not.** Record the finding, the reason,
  and any evidence. The next round's reviewer gets it and may attack it.
- **Re-run the repo's test gate before the round ends.** A fix that breaks a previously
  passing test is a finding of its own: log it and fix it inside the same round rather
  than shipping it into the next.
- **Update the ledger** with the round's findings, severities, dispositions, and
  evidence.

---

## Phase 2: Decide whether to run another round

After each round, exactly one of these applies:

- **Nothing above Nit → `CONVERGED`.** A fresh reviewer, looking at the fixed code with
  the full ledger in hand, found nothing that matters. Go to Phase 3.
- **Anything at Minor or above → run another round.** Including a finding you dismissed:
  the dismissal is a claim, and the next reviewer's job is to test it. Return to Phase 1
  with the next lens.
- **Round cap reached with findings outstanding → `CAPPED`.** Stop and report honestly.
  `CAPPED` is not a pass; the report names exactly what is still open.
- **Thrash detected → `ESCALATED`.** Stop immediately and hand it to the user.

**Thrash** is the failure mode that makes an unbounded loop dangerous, and it has three
recognisable shapes. Any one of them ends the run:

1. **A finding is fixed and then re-raised in a later round** — the fix didn't work, or
   reviewer and fixer disagree about what correct means. Another round won't settle it.
2. **Two rounds' fixes contradict each other** — round N changes a behaviour and round
   N+1 changes it back. The loop is oscillating, not converging.
3. **Findings per round stops falling** while the same area keeps coming up — the change
   has a design problem the round-by-round loop can only keep patching.

Escalating on thrash is correct behaviour, not a failure to finish. Report which shape
fired, the finding(s) involved, and the competing positions, and let the user decide.

---

## Phase 3: Report

The report is the skill's output. Print it as the final message:

```markdown
# Adversarial review — <target>

**Verdict:** <CONVERGED | CAPPED | ESCALATED>
**Rounds:** <n> of <cap>  ·  **Lenses:** <round → lens, …>
**Test gate:** <command> — <result at the end of the run> (baseline: <result at Phase 0>)

<one-sentence rationale for the verdict>

## Convergence
| Round | Lens | Blocker | Major | Minor | Nit | Fixed | Dismissed |
|-------|------|---------|-------|-------|-----|-------|-----------|
| 1 | correctness | 1 | 2 | 1 | 3 | 4 | 0 |
| 2 | data/security | 0 | 0 | 1 | 1 | 1 | 0 |
| 3 | tests | 0 | 0 | 0 | 2 | 2 | 0 |

## Findings
| # | Round | Severity | File:line | Finding | Disposition |
|---|-------|----------|-----------|---------|-------------|

## Dismissed, with reasons
- [#n, Minor] <finding> — <reason>, unchallenged in round <m>

## Still open
- <every unfixed Minor-or-above, or _None._>

## Fixes
- <sha or file> — <what it fixed> (test: <the test that failed first>)
```

Replace an empty section with `_None._` rather than deleting the heading, so one run's
report diffs cleanly against the next.

Then **stop**. This skill reviews and fixes; it does not commit, push, open a PR, or
request review — the caller (`/execute-brd`, `/git-workflow:ship-pr`, or the user) owns
what happens next. Say plainly what the working tree now contains.

---

## Standards & guardrails

- **The empty round has to be real.** Every mechanism here — fresh subagent, reviewer-owns-severity, dismissals visible to the next reviewer — exists to stop a run from ending because everyone got tired. A `CONVERGED` this skill can't defend is worse than a `CAPPED` it can.
- **A fresh subagent per round, every round.** Never reuse the previous round's reviewer, and never review your own fixes. Continuing a subagent that already argued a position gets you that position again.
- **Severity is the reviewer's, dismissal is yours, and both are on the record.** The fixer never downgrades a finding to make a round come out empty.
- **A finding needs a failure scenario.** Inputs or state → wrong output, crash, leak, or missed gate. Unfalsifiable findings are noise and they cost rounds.
- **A behavioural fix ships a test that failed first.** Confirm the failure before the fix, not after.
- **Bound the loop.** Cap the rounds, watch for thrash, and escalate rather than grind — an oscillating loop is a signal about the change, not a reason for one more pass.
- **Attribute failures before owning them.** A gate red on the base branch is not this diff's finding.
- **Report honestly.** `CAPPED` and `ESCALATED` are real outcomes and get named as such; never present either as convergence.
- **No PHI, secrets, or real customer data** in a subagent prompt, the ledger, or the report.
