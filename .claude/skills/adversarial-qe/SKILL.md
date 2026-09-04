---
name: adversarial-qe
description: "QE a change to convergence by driving the running app, not by
  reading the diff: dispatch a fresh QE subagent that clicks through the feature
  in a real browser via Tidewave browser_eval, fix every defect above nit
  severity, then dispatch another — round after round, each round rotating the
  lens (happy path → empty/loading/error states → boundaries and permutations →
  roles and direct-URL access → keyboard/screen-reader) and carrying forward the
  ledger, until a full round of exercising the app surfaces nothing above a nit.
  Caps rounds, escalates on thrash, and reports a round-by-round ledger with a
  CONVERGED / CAPPED / ESCALATED verdict. Never touches QE-owned automation
  repos. TRIGGER when: the user asks to QE something adversarially or to
  convergence, \"click through this until it's clean\", \"test this in the
  browser until only nits are left\", \"exercise the app like QE would\", wants
  repeated browser-driven verification passes, or types /qe:adversarial-qe
  [target]. SKIP when: the verification is of the diff rather than the running
  app (use /code-review:adversarial-review), the check is purely visual/CSS
  (browser_eval is the wrong instrument — hand it to a human), or QE has already
  filed defects to work through (use the repo's failed-qe skill). Args: [target]
  — what to exercise: a URL or route, a BRD slug, a ticket id, or nothing to
  derive it from the current branch's diff."
---

# Adversarial QE

Exercise a change in the running app until a fresh tester runs out of things to find.
Optional argument (route / BRD slug / ticket / round cap): $ARGUMENTS

This is `adversarial-review`'s sibling, and the loop is deliberately identical:

> **dispatch a fresh tester → fix everything above nit → dispatch another fresh tester →
> …** until a full round comes back with nothing above a nit.

What differs is the **instrument**. A code reviewer reads the diff and reasons about what
it will do. A tester here **operates the running application** — clicks the workflow,
reads the page, submits the forms, hits the routes directly — and reports what it
actually did. That catches a different class of defect entirely: the state nobody
rendered, the guard that isn't wired to the route, the form that posts a field the server
never reads, the empty list that shows a spinner forever. In-process tests pass on all of
those.

**Both loops are needed and neither substitutes for the other.** Run the code loop for
what the diff will do, and this one for what the app does.

## The instrument, and its limits

- **Drive the browser through `browser_eval` from the `tidewave-ide` MCP server.** Not
  Playwright, not another browser MCP, not `curl`. The Tidewave browser is the session the
  user is looking at — the shared context is the point.
- **Prefer `browser.snapshot()` and `browser.getBySnapshotRef` over eval scripts** to read
  a page. A snapshot is cheap input; hand-written DOM-scraping scripts are expensive
  output and break on markup that a snapshot reads fine.
- **This skill does not judge design, styling, or CSS.** `browser_eval` is the wrong
  instrument for it, and a round spent on spacing is a round not spent on behaviour.
  Visual and stylistic questions go to a human — say so and move on.
- **Read the server side too.** Tidewave's log tool surfaces the exceptions, warnings, and
  query patterns the UI swallows. A page that renders fine over a stream of server errors
  is a finding; the browser alone won't tell you.
- **Code is still the source of truth for *expected* behaviour.** Read the BRD, the port
  plan, the ticket, or the Rails/legacy surface being replaced before deciding something
  is wrong. "I expected it to work differently" is not a defect.
- **Stuck is a stopping point, not a retry loop.** If a round genuinely cannot drive the
  app — sign-in wall, a control that won't respond, an unreachable page — take one or two
  honest attempts, then **ask the user**. Do not burn a round grinding on the harness.

## Phase −1: Learn this repo, and get the app up

Nothing below hardcodes a stack. Before dispatching anything:

- **Read `AGENTS.md` / `CLAUDE.md`** and the repo's `.claude/skills/` for how this app is
  run locally, how to sign in to the dev environment (most repos document the seeded
  credentials and the 2FA path in a Tidewave or local-setup guide), which roles exist, and
  what its QE handoff contract is (`data-testid` conventions, who owns naming).
- **Confirm the app is actually serving the code under test.** A round driving a stale
  build reports defects that no longer exist and misses the ones that do. Check the app
  responds, and that the change is present on the page.
- **Confirm `browser_eval` works and you are signed in** as a role that can reach the
  surface. Landing on a sign-in page is the single most common reason a round returns
  nothing; resolve it before dispatching, not inside a round.

**QE-owned automation repos are off limits.** This skill exercises the app and fixes the
*application* code. It never creates a branch, commit, or PR in an automation/E2E repo,
never adds or edits a feature file or step definition, and never treats a red automation
run as its own to fix. Where a scenario is missing, describe it for QE — authoring it is
their call. If the repo's own docs say this differently, the repo wins.

---

## Phase 0: Establish the target, the expectations, and the baseline

1. **Resolve what to exercise** from `$ARGUMENTS`:
   - a **URL / route** → exercise it directly
   - a **BRD slug** → read the BRD; its user-facing `BR-n` and primary workflow are the
     expectations, and its routes are the target
   - a **ticket id** → read the ticket and its acceptance criteria
   - **nothing** → derive it from the current branch's diff: which routes, LiveViews /
     controllers / views, and user-facing surfaces did it touch?
2. **Write down the expected behaviour before you touch the app.** A list of concrete,
   checkable expectations — from the BRD's `BR-n` and fit criteria, the ticket's AC, the
   repo's stated conventions, and where the change replaces an existing surface, **the
   behaviour of the surface being replaced**. This list is what "correct" means for the
   whole run. Deciding it *after* seeing the app is how you end up ratifying whatever it
   happens to do.
3. **Rehearse the click-path once, yourself.** Walk the primary workflow and record the
   robust locators (ids, `data-testid`s, labels) and the URLs. Snapshot refs change
   between renders, so a subagent handed refs instead of selectors will fail on markup
   that is fine. This rehearsal is also where a broken harness surfaces — before it costs
   a round. Revert anything you changed while rehearsing.
4. **Note the data you have.** Which practice/tenant, which roles, whether the surface has
   both a populated and an empty case available. A missing empty-state fixture is a gap in
   the run, not evidence the empty state works — say which you could actually reach.
5. **Pick the round cap.** Default **5**; `$ARGUMENTS` may override. Hitting it reports
   `CAPPED`, which is not a pass.
6. **Open the ledger** — one row per defect, for the whole run:

   | # | Round | Severity | Where | Steps to reproduce | Observed vs expected | Disposition |
   |---|-------|----------|-------|--------------------|----------------------|-------------|

   Subagents are fresh each round; the ledger is what carries across them, and it is the
   final report.

---

## Phase 1: Run a round

Repeat until Phase 2 says stop.

### 1a. Dispatch a fresh tester

One QE subagent per round, with no context from previous rounds beyond the ledger you
hand it. **The tester is never the fixer** — whoever wrote the fix knows what it was
supposed to do and will drive the app the way it expects to work, which is precisely the
path that already passes.

The prompt carries, every round:

- **How to reach the surface** — the URL, the sign-in path, the role to use, and the
  robust locators from the Phase 0 rehearsal (never snapshot refs).
- **The expectation list** from Phase 0. A tester without it can only report crashes.
- **This round's lens** (1b), as the primary focus, not a restriction. A tester that
  trips over a tenancy leak while assigned the keyboard lens reports it immediately.
- **The ledger so far**, with each dismissal's reason.
- **The severity ladder** (1c). Severity is the **tester's** call.
- **The evidence standard:** every defect needs the click-path that produced it, what was
  observed, and what was expected — enough for someone else to reproduce it in one go. A
  defect nobody else can reproduce costs a round.
- **The instrument rules** from the top of this skill: `browser_eval` only, snapshots over
  eval scripts, no design/CSS judgments, check the server logs, ask rather than grind.

### 1b. Rotate the lens

Adapt to the surface — a read-only page has no validation lens — and log which lens each
round used:

| Round | Primary lens |
|---|---|
| 1 | **Primary workflow** — the main journey end to end, exactly as a user would. Every step reachable, no dead ends, the result actually persisted (reload and confirm — a UI that updates without saving is the classic pass-in-tests defect) |
| 2 | **States** — empty, loading, error, validation, partial data, and the "nothing selected yet" case. Submit a form blank; submit it invalid; trigger the failure path deliberately and read what the user is told |
| 3 | **Boundaries and permutations** — the edges of each input (min, max, just past, zero, one, many), first/last row, page turns, sort and filter combinations, and the same journey from a second entry point |
| 4 | **Roles and access** — every role in the expectation list, including one that should be denied. Hit the route **directly by URL** rather than through the nav, and try a record id belonging to another tenant. A guard on the button with none on the route is a real and common defect |
| 5 | **Keyboard and screen-reader operability** — tab order, focus visibility, focus after a modal opens and closes, Enter/Space/Escape, and whether errors are announced. Only a real browser can see this, which is why it belongs here rather than in the code loop |

Past the list, rotate back to **Primary workflow** with an instruction to reach it a way
earlier rounds didn't (a different entry point, a reload mid-flow, the back button, a
second tab).

### 1c. Severity ladder

| Severity | Definition | Effect |
|---|---|---|
| **Blocker** | The workflow can't be completed, data is lost or not persisted, the wrong tenant's or role's data is visible, or the page errors out on a reachable path. | Must fix. Blocks convergence. |
| **Major** | A state is missing or wrong (no empty state, a spinner that never resolves, a validation error the user can't act on), a guard is missing from the route, or a boundary produces the wrong result. | Must fix. Blocks convergence. |
| **Minor** | Genuine but low-impact — an awkward error message, a stale label, focus landing somewhere unhelpful, a narrow edge case. | Must fix or explicitly dismiss with a reason. Blocks convergence. |
| **Nit** | Wording preference, subjective polish, anything visual or stylistic (which this skill does not judge at all). | Does **not** block. Note and move on. |

**The fixer never re-grades a defect.** Believing a Major is really a Nit is a
*disagreement*: it goes in the ledger as dismissed-with-reason, still blocks the round,
and hands the next tester the reason to attack. A loop whose empty round was reached by
downgrading is worthless.

### 1d. Fix what the round found

Fix in the **application** code, yourself.

- **Add the regression coverage where the repo says it belongs** — in most repos that is
  an in-process test (controller/LiveView/request spec), not a browser test, and browser
  testing is QE's layer. Follow the repo's own testing strategy from Phase −1. Where a
  defect genuinely can only be caught in a real browser, that is QE's coverage to author:
  describe the scenario for them rather than writing it.
- **A behavioural fix ships a test that fails first.** Watch it fail, then fix.
- **Re-drive the exact click-path from the defect** after fixing, and record that you
  did. "Should be fixed" is not verification — the point of this skill is that a claim
  about the running app is checked against the running app.
- **Dismissing is allowed; dismissing silently is not.** Record the reason; the next
  tester gets it and may attack it.
- **Run the repo's test gate** before the round ends. A fix that reddens a passing test
  is a finding of its own — handle it in the same round.
- **`data-testid` naming is QE's.** Add the attribute where it's missing on an element QE
  will hook, reusing the legacy/Rails value when one exists; where there is none, use the
  repo's documented placeholder and list it for QE. Never invent a final name.
- **Update the ledger.**

---

## Phase 2: Decide whether to run another round

- **Nothing above Nit → `CONVERGED`.** A fresh tester, driving the fixed app with the
  full ledger in hand, found nothing that matters. Go to Phase 3.
- **Anything at Minor or above → run another round** with the next lens. A dismissal is a
  claim; the next tester tests it.
- **Cap reached with defects outstanding → `CAPPED`.** Not a pass. Name what's open.
- **Thrash → `ESCALATED`.** Stop and hand it to the user. The three shapes:
  1. **A defect is fixed and reappears in a later round** — the fix didn't hold, or
     tester and fixer disagree about correct.
  2. **Two rounds' fixes contradict each other** — the loop is oscillating.
  3. **Defects per round stops falling** while the same surface keeps failing — a design
     problem the loop can only keep patching.

  Also escalate when a round **couldn't drive the app at all** (sign-in, environment,
  data). That is not a clean round and must never be counted as one.

---

## Phase 3: Report

```markdown
# Adversarial QE — <target>

**Verdict:** <CONVERGED | CAPPED | ESCALATED>
**Rounds:** <n> of <cap>  ·  **Lenses:** <round → lens, …>
**Exercised:** <routes/URLs> as <roles>  ·  **Data:** <tenant/practice, populated + empty availability>
**Test gate:** <command> — <result> (baseline: <result at Phase 0>)

<one-sentence rationale for the verdict>

## Convergence
| Round | Lens | Blocker | Major | Minor | Nit | Fixed | Dismissed |
|-------|------|---------|-------|-------|-----|-------|-----------|

## Defects
| # | Round | Severity | Where | Steps to reproduce | Observed vs expected | Disposition |
|---|-------|----------|-------|--------------------|----------------------|-------------|

## Dismissed, with reasons
- [#n, Minor] <defect> — <reason>, unchallenged in round <m>

## Still open
- <every unfixed Minor-or-above, or _None._>

## Not covered
- <expectations no round could reach, and why — missing fixture, unavailable role, no error path to trigger>

## For QE
- **Scenarios to add:** <what a round found that QE's suite doesn't cover>, or _None._
- **`data-testid` to name:** <placeholders left for QE>, or _None._

## Handed to a human
- <visual/CSS questions and anything the harness blocked>, or _None._
```

Replace an empty section with `_None._` rather than deleting the heading.

Then **stop**. This skill exercises and fixes; it does not commit, push, open a PR, or
request review — the caller (`/execute-brd`, or the user) owns what comes next.

---

## Standards & guardrails

- **The empty round has to be real.** A `CONVERGED` reached by downgrading defects, by a round that never reached the page, or by testing only the path the fix was written for is a false pass — and a false pass here is worse than no run, because it is the last gate before a human.
- **A fresh subagent per round, and the tester is never the fixer.** Continuing the previous round's tester gets you the previous round's conclusions.
- **Report only what the app did.** Every defect carries a reproducible click-path and an observed-vs-expected. No reasoning about what the code probably does — that is the code loop's job.
- **Expectations come from the BRD / ticket / replaced surface, and are written down before the app is touched.** Otherwise the run ratifies whatever it finds.
- **Verify each fix by re-driving the failing path**, and say so in the ledger.
- **`browser_eval` only; snapshots over eval scripts; no design/CSS judgments; check the server logs.**
- **Ask when the harness blocks you** after one or two attempts. A round that couldn't drive the app is never a clean round.
- **Bound the loop.** Cap the rounds, watch for thrash, escalate rather than grind.
- **Automation repos and `data-testid` names are QE's.** Describe, don't author. Never open a PR in an automation repo.
- **No PHI, secrets, or real patient/customer data** in a subagent prompt, the ledger, the report, or any browser script. Report record ids, counts, states, and timestamps — never names, DOB, MRN, contact details, or clinical free text, and never dump a whole record.
