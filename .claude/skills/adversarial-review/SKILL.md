---
name: adversarial-review
description: "Review a change to convergence instead of once: dispatch a fresh
  review subagent, hand its findings to a separate fixer subagent, then dispatch
  another reviewer — round after round, each round carrying forward what was
  already fixed and dismissed, until a full round surfaces nothing above a nit.
  Rotates the review lens per round so a later round isn't a rerun of the first,
  caps rounds so a reviewer/fixer standoff can't loop forever, and reports a
  round-by-round ledger with an explicit CONVERGED / CAPPED / ESCALATED verdict.
  Orchestrate-only: the driving session dispatches and decides, every round's
  state crosses on disk, so round count isn't bounded by the transcript. Repo-
  and stack-agnostic — discovers the repo's own test and lint commands rather
  than assuming a toolchain. TRIGGER when: the user asks to review something
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

> **dispatch a fresh reviewer → hand its findings to a fresh fixer → dispatch another
> fresh reviewer → …** until a full round comes back with nothing above a nit.

Three properties make it work, and all three are easy to lose:

- **The reviewer is never the fixer.** Each round dispatches a **new reviewer subagent
  with no memory of the previous rounds' reasoning**, and the fixing goes to a *different*
  subagent. Whoever just wrote a fix is the worst possible judge of it — they know what
  they intended, so they read the intent rather than the code. A fresh reviewer reads only
  what is there.
- **Rounds don't repeat themselves.** Each round gets the *ledger* (what was already
  found, fixed, and dismissed, and why) and a **different lens**, so round 3 is looking
  somewhere round 1 didn't rather than re-reporting round 1's list.
- **The driving session stays thin.** You orchestrate; the subagents work. State crosses
  rounds as a document on disk, not as context you carry.

## You orchestrate. You do not work.

This is a many-round loop, and the thing that ends one early is the driving session
filling up. So the expensive work — reading the diff, reading source, editing, running
the test gate — happens **inside subagents**, and only a short structured summary comes
back to you.

**In this skill you never:**

- read the diff, a source file, or a findings file
- edit code, write a test, or run the test/lint gate
- paste a subagent's full output into your own reasoning or the transcript
- hand a subagent the *contents* of the ledger — you hand it the **path**

**You only:** dispatch subagents, read their short summaries, decide the verdict
(another round / CONVERGED / CAPPED / ESCALATED), and print the final report.

If you find yourself opening a file to check a finding, the round is being run wrong.
Dispatch a subagent to check it.

### The run directory

Everything that crosses a round lives in one directory, outside the repo so no repo needs
to gitignore it:

```
${TMPDIR:-/tmp}/adversarial-review/<slug>/
  brief.md              # written once by the scout: target, gates, commands, baseline
  handoff.md            # THE document each round reads — ledger + state, rewritten per round
  round-<n>-findings.md # the reviewer's full findings, with evidence
  round-<n>-fixes.md    # the fixer's account: what changed, which test failed first, dismissals
```

`<slug>` is the target reduced to something filesystem-safe — `pr-1234`,
the branch name with `/` replaced, or `working-tree`. Deterministic, so a re-run against
the same target lands in the same place. Create it before Phase 0 and print the path in
the report; it holds the evidence the report only summarises.

**`handoff.md` is the memory of the run.** Subagents are fresh every round and you are
deliberately ignorant of the detail, so this file is the only continuity. It carries the
full ledger, every dismissal with its reason, and the state of the working tree. Each
round's fixer rewrites it as the round's last act — not you.

### What subagents return to you

Every subagent prompt ends with a hard return contract. Nothing longer comes back, and a
subagent that returns prose instead gets its summary re-requested, not read.

A reviewer returns:

```
ROUND: <n>  LENS: <lens>
COUNTS: blocker <n>, major <n>, minor <n>, nit <n>
- [<Severity>] <file:line> — <one line>
- …
RE-RAISED: <ledger #s the round re-raised, or none>
FINDINGS_FILE: <path>
```

A fixer returns:

```
ROUND: <n>
FIXED: <n>  DISMISSED: <n>  NEW_FAILURES: <n>
- #<n> <Severity> fixed — test that failed first: <test id>
- #<n> <Severity> dismissed — <reason, one line>
TEST GATE: <command> — <result>
FIXES_FILE: <path>   HANDOFF: updated
```

That is roughly twenty lines of transcript per round, whatever the diff's size — which is
what lets the loop run its full cap.

---

## Phase −1/0: Dispatch the scout

Do **not** learn the repo or read the diff yourself. Dispatch one **scout subagent** that
does all of it and writes `brief.md`. Its prompt:

- **Learn this repo.** Read `AGENTS.md` / `CLAUDE.md` at the root plus any nested ones
  covering the paths in the diff, and the repo's own skills under `.claude/skills/`. These
  are the authority on the test command, the format/lint command, the full pre-commit gate,
  the commit protocol, the test-file mirror map, and any repo-specific review gates
  (multi-tenancy scoping, PHI handling, telemetry, N+1 coverage, `data-testid`
  requirements). A repo-local rule outranks a generic instinct.
- **Resolve the target** from `$ARGUMENTS`:
  - a **PR number / URL** → `gh pr diff <pr>`, and note the head branch
  - a **branch** → its diff against the repo's default branch
  - a **path** → the changes under it, or the file set itself if there is no diff
  - **nothing** → the current working diff (`git diff` + `git diff --cached`), falling
    back to the branch's diff against the default branch if the tree is clean
- **Read the diff and record the surfaces touched** (context/model,
  web/controller/view, migration, worker, config) — these drive the lens rotation.
- **Establish a green baseline.** Run the repo's test gate and record the result,
  naming any gate already red on the base branch. A pre-existing failure is not a finding
  against this diff, and mistaking it for one wastes a round.
- **Write `brief.md`**: the target and how to obtain the diff, the base to compare
  against, the surfaces touched, the test/lint/pre-commit commands, the repo's review
  gates **stated verbatim**, and the baseline result.

The review gates matter most: a reviewer that hasn't been told the repo requires a
cross-tenant test cannot report its absence. They go into `brief.md`, and every round's
reviewer is pointed at it.

The scout returns only: surfaces touched, the test command, the baseline result, and any
pre-existing failure. Then:

1. **Pick the round cap.** Default **5**. `$ARGUMENTS` may override it. The cap exists to
   bound a standoff, not to be hit — a run that hits it reports `CAPPED`, which is not a
   pass.
2. **Seed `handoff.md`** — have the scout write it empty-but-structured: an empty ledger
   table, the round cap, and a "no rounds run yet" state.

   | # | Round | Severity | File:line | Finding | Disposition | Evidence |
   |---|-------|----------|-----------|---------|-------------|----------|

---

## Phase 1: Run a round

Repeat this phase until Phase 2 says stop. Each round is **two subagents, dispatched in
sequence**.

### 1a. Dispatch a fresh reviewer

One review subagent per round, with no context from previous rounds beyond the files you
point it at.

Where the environment has a review skill that already encodes the repo's standards
(`/code-review` at a given effort, or a repo-local review skill found by the scout), have
the subagent run **that** rather than reinventing a checklist — it will apply conventions
this skill doesn't know about. Otherwise brief it directly.

The prompt carries, every round:

- **The paths** to `brief.md` and `handoff.md`, with an instruction to read both first.
  Never the contents — the whole point is that they don't pass through you.
- **This round's lens** (1b) — where to look, stated as the round's *primary* focus and
  not as a restriction. A reviewer that finds a data-loss bug while assigned the
  "tests and coverage" lens reports it.
- **The severity ladder** (1c), and an instruction to assign a severity to every
  finding. Severity is the **reviewer's** call.
- **The standing instruction to verify before reporting.** A finding needs a concrete
  failure scenario: inputs or state → the wrong output, crash, leak, or missed gate. A
  finding that can't be stated that way is speculation, and speculation costs a round.
- **Where to write** — `round-<n>-findings.md`, one section per finding with its full
  evidence and failure scenario — and **the return contract** above. The file is the
  finding; the return is the index.

**On re-raising a dismissed finding:** the reviewer may — and should — re-raise
something the ledger records as dismissed, **but only by showing the dismissal reason is
wrong**, and it must say so explicitly (that is what `RE-RAISED:` is for). Suppressing
that outright would let a bad dismissal end the run, which is the failure mode this whole
skill exists to prevent. A re-raise that just restates the original finding without
engaging the reason is noise; say so in the ledger and move on.

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

**Nobody downstream re-grades a finding.** The fixer believing a Minor is really a Nit is
a *disagreement*, and it goes in the ledger as one — dismissed-with-reason, which still
blocks the round and hands the next reviewer the reason to attack. Letting the fixer
downgrade its way to an empty round is exactly how this loop gets faked, and the whole
value of the run is that the empty round is real. Say this in the fixer's prompt every
round.

### 1d. Dispatch a fresh fixer

A **separate subagent** works the round's findings. You do not fix, and the reviewer does
not fix. Its prompt carries the paths to `brief.md`, `handoff.md`, and
`round-<n>-findings.md`, plus:

- **Work the findings in severity order.**
- **A fix for a behavioural finding ships a test that fails first.** Write the test,
  watch it fail against the current code, then fix until it passes, and record the test's
  identifier. A regression test that never failed is asserting nothing.
- **Follow the repo's test-file placement** from `brief.md` — add to the mirroring test
  file, never a new variant beside it.
- **Dismissing is allowed; dismissing silently is not.** Record the finding, the reason,
  and any evidence. The next round's reviewer gets it and may attack it.
- **Never re-grade a severity** (1c).
- **Re-run the repo's test gate before finishing.** A fix that breaks a previously
  passing test is a finding of its own: fix it inside the same round rather than shipping
  it into the next, and report it as `NEW_FAILURES`.
- **Write `round-<n>-fixes.md`**, then **rewrite `handoff.md`** — every finding to date
  with its severity, disposition, evidence, and for each dismissal the reason; the state
  of the working tree; the rounds and lenses used so far. This is the fixer's last act and
  the next round depends on it.
- **The return contract** above.

---

## Phase 2: Decide whether to run another round

Decide from the two summaries alone. After each round, exactly one of these applies:

- **Nothing above Nit → `CONVERGED`.** A fresh reviewer, looking at the fixed code with
  the full ledger in hand, found nothing that matters. Go to Phase 3.
- **Anything at Minor or above → run another round.** Including a finding the fixer
  dismissed: the dismissal is a claim, and the next reviewer's job is to test it. Return
  to Phase 1 with the next lens.
- **Round cap reached with findings outstanding → `CAPPED`.** Stop and report honestly.
  `CAPPED` is not a pass; the report names exactly what is still open.
- **Thrash detected → `ESCALATED`.** Stop immediately and hand it to the user.

**Thrash** is the failure mode that makes an unbounded loop dangerous, and it has three
recognisable shapes. Any one of them ends the run:

1. **A finding is fixed and then re-raised in a later round** — the fix didn't work, or
   reviewer and fixer disagree about what correct means. Another round won't settle it.
   The reviewer's `RE-RAISED:` line against a finding the ledger marks *fixed* is the
   tell.
2. **Two rounds' fixes contradict each other** — round N changes a behaviour and round
   N+1 changes it back. The loop is oscillating, not converging.
3. **Findings per round stops falling** while the same area keeps coming up — the change
   has a design problem the round-by-round loop can only keep patching.

Escalating on thrash is correct behaviour, not a failure to finish. Report which shape
fired, the finding(s) involved, and the competing positions, and let the user decide.

---

## Phase 3: Report

Compose the report from the round summaries you already hold — do **not** read the run
directory back to write it. The report is a summary by design; it points at the run
directory for the evidence. Print it as the final message:

```markdown
# Adversarial review — <target>

**Verdict:** <CONVERGED | CAPPED | ESCALATED>
**Rounds:** <n> of <cap>  ·  **Lenses:** <round → lens, …>
**Test gate:** <command> — <result at the end of the run> (baseline: <result from the scout>)
**Run directory:** <path> — full findings, fixes, and ledger

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
what happens next. Say plainly what the working tree now contains. Leave the run directory
in place; it is temporary storage the user may want to read, and nothing else depends on
cleaning it up.

---

## Standards & guardrails

- **The empty round has to be real.** Every mechanism here — fresh subagent, reviewer-owns-severity, separate fixer, dismissals visible to the next reviewer — exists to stop a run from ending because everyone got tired. A `CONVERGED` this skill can't defend is worse than a `CAPPED` it can.
- **A fresh subagent per round, every round.** Never reuse the previous round's reviewer, and never let a subagent review its own fixes. Continuing a subagent that already argued a position gets you that position again.
- **Orchestrate, don't work.** You dispatch and decide. Reading the diff, editing code, or running the gate yourself is what makes long runs impossible — and long runs are the entire point.
- **State crosses on disk, not in context.** Subagents get the path to `handoff.md`, never its contents; they return the short contract, never prose.
- **Severity is the reviewer's, dismissal is the fixer's, and both are on the record.** Nobody downgrades a finding to make a round come out empty.
- **A finding needs a failure scenario.** Inputs or state → wrong output, crash, leak, or missed gate. Unfalsifiable findings are noise and they cost rounds.
- **A behavioural fix ships a test that failed first.** Confirm the failure before the fix, not after.
- **Bound the loop.** Cap the rounds, watch for thrash, and escalate rather than grind — an oscillating loop is a signal about the change, not a reason for one more pass.
- **Attribute failures before owning them.** A gate red on the base branch is not this diff's finding.
- **Report honestly.** `CAPPED` and `ESCALATED` are real outcomes and get named as such; never present either as convergence.
- **No PHI, secrets, or real customer data** in a subagent prompt, the run directory, or the report.
