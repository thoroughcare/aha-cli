---
name: adversarial-qe
description: "QE a change to convergence by driving the running app, not by
  reading the diff: dispatch a fresh QE subagent that clicks through the feature
  in a real browser via Tidewave browser_eval, hand its defects to a separate
  fixer subagent, then dispatch another — round after round, each round rotating
  the lens (happy path → empty/loading/error states → boundaries and
  permutations → roles and direct-URL access → keyboard/screen-reader) and
  carrying the ledger forward on disk, until a full round of exercising the app
  surfaces nothing above a nit. Orchestrate-only, so the driving session stays
  thin and the browser traffic stays inside subagents. Caps rounds, escalates on
  thrash, and reports a round-by-round ledger with a CONVERGED / CAPPED /
  ESCALATED verdict. Never touches QE-owned automation repos. TRIGGER when: the
  user asks to QE something adversarially or to convergence, \"click through
  this until it's clean\", \"test this in the browser until only nits are
  left\", \"exercise the app like QE would\", wants repeated browser-driven
  verification passes, or types /qe:adversarial-qe [target]. SKIP when: the
  verification is of the diff rather than the running app (use
  /code-review:adversarial-review), the check is purely visual/CSS (browser_eval
  is the wrong instrument — hand it to a human), or QE has already filed defects
  to work through (use the repo's failed-qe skill). Args: [target] — what to
  exercise: a URL or route, a BRD slug, a ticket id, or nothing to derive it
  from the current branch's diff."
---

# Adversarial QE

Exercise a change in the running app until a fresh tester runs out of things to find.
Optional argument (route / BRD slug / ticket / round cap): $ARGUMENTS

This is `adversarial-review`'s sibling, and the loop is deliberately identical:

> **dispatch a fresh tester → hand its defects to a fresh fixer → dispatch another fresh
> tester → …** until a full round comes back with nothing above a nit.

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
- **This skill judges whether the page works, not whether it looks good.** A visual
  problem that blocks or misleads a user — a control hidden behind an overlap, text
  truncated past legibility, a broken layout — is a defect and gets reported. Subjective
  polish is not: spacing, palette, visual hierarchy, "this feels cramped" go to a human,
  and a round spent on them is a round not spent on behaviour.
- **Read the server side too.** Tidewave's log tool surfaces the exceptions, warnings, and
  query patterns the UI swallows. A page that renders fine over a stream of server errors
  is a finding; the browser alone won't tell you.
- **Code is still the source of truth for *expected* behaviour.** Read the BRD, the port
  plan, the ticket, or the Rails/legacy surface being replaced before deciding something
  is wrong. "I expected it to work differently" is not a defect.
- **Stuck is a stopping point, not a retry loop.** If a round genuinely cannot drive the
  app — sign-in wall, a control that won't respond, an unreachable page — take one or two
  honest attempts, then **ask the user**. Do not burn a round grinding on the harness.

### Browser economy — spend the expensive instrument deliberately

A Tidewave round is the most token-expensive thing either adversarial skill does, and
**images are why**: a screenshot costs orders of magnitude more than an accessibility
snapshot, and vision mode applies that cost to every step. The answer is not to stop
looking. **Seeing the page is often the whole point of a QE round** — a layout that hides a
control, an overlap that makes something unclickable, a state that renders wrong are real
defects a snapshot reports as fine. Use vision where it earns its cost, and don't use it
where it doesn't. Every prompt this skill dispatches carries the split:

- **Structural and behavioural questions go to `browser.snapshot()`.** Whether a control
  exists, is disabled, holds the right value, or produced the right message is in the
  snapshot, and `browser.getBySnapshotRef` is the reliable way to locate an element. That
  is most steps in most rounds, and it is the better instrument for them regardless of
  cost — hand-written DOM-scraping eval scripts are expensive output and break on markup a
  snapshot reads fine.
- **Questions about what the user actually sees go to a screenshot.** Layout, overlap,
  truncation, whether the page is visibly broken, whether a control is reachable at all.
  Don't talk yourself out of looking when looking is the check — a round that never looks
  can't find this class of defect, and it is exactly the class in-process tests miss.

What burns a quota is the reflexive capture, not the justified one:

- **Capture on state change, not per action.** A click that navigates or re-renders earns
  a look; filling three fields in a form does not earn three.
- **Never poll by re-capturing in a loop.** Waiting for a load or a stream that way is the
  fastest way to burn a quota. Wait on the condition, then read once.
- **Batch your questions against one capture.** Check everything the current state can
  tell you before advancing; re-reading a page costs the same as reading it the first
  time.
- **Say what a screenshot was for.** The round reports its capture count, so a round that
  looked twenty times should be able to say why. This is a prompt for deliberateness, not
  a budget.

Keeping the browser work inside subagents (below) is what stops this traffic from landing
in the driving session at all; these rules are what keep it small even there.

## You orchestrate. You do not work.

This is a many-round loop, and the thing that ends one early is the driving session
filling up — with browser output faster than anything else. So the expensive work —
driving the app, reading source, editing, running the test gate — happens **inside
subagents**, and only a short structured summary comes back to you.

**In this skill you never:**

- call `browser_eval` yourself, or read a snapshot, a screenshot, or a log dump
- read a source file or a defects file
- edit code, write a test, or run the test gate
- paste a subagent's full output into your own reasoning or the transcript
- hand a subagent the *contents* of the ledger — you hand it the **path**

**You only:** dispatch subagents, read their short summaries, decide the verdict
(another round / CONVERGED / CAPPED / ESCALATED), and print the final report.

The one exception is the harness: when a subagent reports it could not drive the app, you
may confirm that yourself before deciding whether to escalate — a stuck harness is a
decision, not work.

### The run directory

Everything that crosses a round lives in one directory, outside the repo so no repo needs
to gitignore it:

```
${TMPDIR:-/tmp}/adversarial-qe/<slug>/
  brief.md              # written once by the scout: target, expectations, locators, sign-in, baseline
  handoff.md            # THE document each round reads — ledger + state, rewritten per round
  round-<n>-defects.md  # the tester's full defects, with click-paths and observed-vs-expected
  round-<n>-fixes.md    # the fixer's account: what changed, which test failed first, re-drive result
```

`<slug>` is the target reduced to something filesystem-safe — the route, the BRD slug, or
the ticket id. Deterministic, so a re-run against the same target lands in the same place.
Create it before Phase 0 and print the path in the report; it holds the evidence the
report only summarises.

**`handoff.md` is the memory of the run.** Subagents are fresh every round and you are
deliberately ignorant of the detail, so this file is the only continuity. It carries the
full ledger, every dismissal with its reason, which expectations have been exercised, and
the state of the working tree. Each round's fixer rewrites it as the round's last act —
not you.

### What subagents return to you

Every subagent prompt ends with a hard return contract. Nothing longer comes back, and a
subagent that returns prose instead gets its summary re-requested, not read.

A tester returns:

```
ROUND: <n>  LENS: <lens>
DROVE: <routes/URLs> as <role>   CAPTURES: <n snapshots, n screenshots — what the screenshots were for>
COUNTS: blocker <n>, major <n>, minor <n>, nit <n>
- [<Severity>] <where> — <one line: observed vs expected>
- …
RE-RAISED: <ledger #s the round re-raised, or none>
UNREACHABLE: <expectations no step could reach, and why, or none>
DEFECTS_FILE: <path>
```

A fixer returns:

```
ROUND: <n>
FIXED: <n>  DISMISSED: <n>  NEW_FAILURES: <n>
- #<n> <Severity> fixed — test that failed first: <test id> — re-drove click-path: <pass/fail>
- #<n> <Severity> dismissed — <reason, one line>
TEST GATE: <command> — <result>
FOR_QE: <scenarios / testid placeholders, or none>
FIXES_FILE: <path>   HANDOFF: updated
```

That is roughly twenty-five lines of transcript per round, whatever the surface's size —
which is what lets the loop run its full cap.

---

## Phase −1/0: Dispatch the scout

Do **not** learn the repo, sign in, or rehearse the click-path yourself. Dispatch one
**scout subagent** that does all of it and writes `brief.md`. Its prompt:

- **Learn this repo.** Read `AGENTS.md` / `CLAUDE.md` and the repo's `.claude/skills/` for
  how this app is run locally, how to sign in to the dev environment (most repos document
  the seeded credentials and the 2FA path in a Tidewave or local-setup guide), which roles
  exist, where regression coverage belongs, and what its QE handoff contract is
  (`data-testid` conventions, who owns naming).
- **Confirm the app is serving the code under test.** A round driving a stale build
  reports defects that no longer exist and misses the ones that do. Check the app responds
  and that the change is present on the page.
- **Confirm `browser_eval` works and sign in** as a role that can reach the surface.
  Landing on a sign-in page is the single most common reason a round returns nothing;
  resolve it here, not inside a round.
- **Resolve what to exercise** from `$ARGUMENTS`:
  - a **URL / route** → exercise it directly
  - a **BRD slug** → read the BRD; its user-facing `BR-n` and primary workflow are the
    expectations, and its routes are the target
  - a **ticket id** → read the ticket and its acceptance criteria
  - **nothing** → derive it from the current branch's diff: which routes, LiveViews /
    controllers / views, and user-facing surfaces did it touch?
- **Write down the expected behaviour before touching the app.** A list of concrete,
  checkable expectations — from the BRD's `BR-n` and fit criteria, the ticket's AC, the
  repo's stated conventions, and where the change replaces an existing surface, **the
  behaviour of the surface being replaced**. This list is what "correct" means for the
  whole run. Deciding it *after* seeing the app is how you end up ratifying whatever it
  happens to do.
- **Rehearse the primary click-path once** and record **robust locators** (ids,
  `data-testid`s, labels) and URLs. Snapshot refs change between renders, so a tester
  handed refs instead of selectors will fail on markup that is fine. This rehearsal is
  also where a broken harness surfaces — before it costs a round. Revert anything changed
  while rehearsing.
- **Note the data available.** Which practice/tenant, which roles, whether the surface has
  both a populated and an empty case. A missing empty-state fixture is a gap in the run,
  not evidence the empty state works.
- **Run the repo's test gate** for a baseline, naming any gate already red on the base
  branch.
- **Obey the browser-economy rules above** — the rehearsal is one pass, not an
  exploration.
- **Write `brief.md`**: target and routes, sign-in path and role, the expectation list,
  the robust locators, the data available, the test/lint commands, the repo's QE gates
  verbatim, and the baseline result.

The scout returns only: the routes and role, the count of expectations recorded, the data
available, the baseline result, and any harness problem. Then:

1. **Pick the round cap.** Default **5**; `$ARGUMENTS` may override. Hitting it reports
   `CAPPED`, which is not a pass.
2. **Seed `handoff.md`** — have the scout write it empty-but-structured: an empty ledger
   table, the round cap, the expectation list, and a "no rounds run yet" state.

   | # | Round | Severity | Where | Steps to reproduce | Observed vs expected | Disposition |
   |---|-------|----------|-------|--------------------|----------------------|-------------|

**QE-owned automation repos are off limits** for every subagent this skill dispatches.
This skill exercises the app and fixes the *application* code. It never creates a branch,
commit, or PR in an automation/E2E repo, never adds or edits a feature file or step
definition, and never treats a red automation run as its own to fix. Where a scenario is
missing, describe it for QE — authoring it is their call. If the repo's own docs say this
differently, the repo wins.

---

## Phase 1: Run a round

Repeat until Phase 2 says stop. Each round is **two subagents, dispatched in sequence**.

### 1a. Dispatch a fresh tester

One QE subagent per round, with no context from previous rounds beyond the files you point
it at. **The tester is never the fixer** — whoever wrote the fix knows what it was
supposed to do and will drive the app the way it expects to work, which is precisely the
path that already passes.

The prompt carries, every round:

- **The paths** to `brief.md` and `handoff.md`, with an instruction to read both first —
  the sign-in path, the role, and the robust locators are in `brief.md`. Never the
  contents; the whole point is that they don't pass through you.
- **This round's lens** (1b), as the primary focus, not a restriction. A tester that
  trips over a tenancy leak while assigned the keyboard lens reports it immediately.
- **The severity ladder** (1c). Severity is the **tester's** call.
- **The evidence standard:** every defect needs the click-path that produced it, what was
  observed, and what was expected — enough for someone else to reproduce it in one go. A
  defect nobody else can reproduce costs a round.
- **The instrument rules and the browser-economy rules** from the top of this skill,
  verbatim: `browser_eval` only, snapshots for structure and vision for what the user
  sees, no polling, no subjective-polish judgments, check the server logs, ask rather than
  grind.
- **Where to write** — `round-<n>-defects.md`, one section per defect with its full
  click-path and observed-vs-expected — and **the return contract** above. The file is the
  defect; the return is the index.

**On re-raising a dismissed defect:** the tester may — and should — re-raise something the
ledger records as dismissed, **but only by showing the dismissal reason is wrong**, and it
must say so explicitly (that is what `RE-RAISED:` is for). Suppressing that outright would
let a bad dismissal end the run, which is the failure mode this whole skill exists to
prevent.

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
| **Nit** | Wording preference, or subjective visual polish — spacing, palette, hierarchy — which this skill doesn't judge. A visual problem that blocks or misleads a user is not a nit; grade it on its effect. | Does **not** block. Note and move on. |

**Nobody downstream re-grades a defect.** The fixer believing a Major is really a Nit is a
*disagreement*: it goes in the ledger as dismissed-with-reason, still blocks the round,
and hands the next tester the reason to attack. A loop whose empty round was reached by
downgrading is worthless. Say this in the fixer's prompt every round.

### 1d. Dispatch a fresh fixer

A **separate subagent** works the round's defects, in the **application** code. You do not
fix, and the tester does not fix. Its prompt carries the paths to `brief.md`,
`handoff.md`, and `round-<n>-defects.md`, plus:

- **Add the regression coverage where the repo says it belongs** — in most repos that is
  an in-process test (controller/LiveView/request spec), not a browser test, and browser
  testing is QE's layer. Follow the repo's own testing strategy from `brief.md`. Where a
  defect genuinely can only be caught in a real browser, that is QE's coverage to author:
  describe the scenario for them rather than writing it.
- **A behavioural fix ships a test that fails first.** Watch it fail, then fix, and record
  the test's identifier.
- **Re-drive the exact click-path from the defect** after fixing, obeying the
  browser-economy rules, and record the result. "Should be fixed" is not verification —
  the point of this skill is that a claim about the running app is checked against the
  running app.
- **Dismissing is allowed; dismissing silently is not.** Record the reason; the next
  tester gets it and may attack it.
- **Never re-grade a severity** (1c).
- **Run the repo's test gate** before finishing. A fix that reddens a passing test is a
  finding of its own — handle it in the same round and report it as `NEW_FAILURES`.
- **`data-testid` naming is QE's.** Add the attribute where it's missing on an element QE
  will hook, reusing the legacy/Rails value when one exists; where there is none, use the
  repo's documented placeholder and list it for QE. Never invent a final name.
- **Never touch a QE-owned automation repo** (Phase 0).
- **Write `round-<n>-fixes.md`**, then **rewrite `handoff.md`** — every defect to date
  with its severity, disposition, evidence, and for each dismissal the reason; which
  expectations have now been exercised; the state of the working tree; the rounds and
  lenses used so far. This is the fixer's last act and the next round depends on it.
- **The return contract** above.

---

## Phase 2: Decide whether to run another round

Decide from the two summaries alone.

- **Nothing above Nit → `CONVERGED`.** A fresh tester, driving the fixed app with the
  full ledger in hand, found nothing that matters. Go to Phase 3.
- **Anything at Minor or above → run another round** with the next lens. A dismissal is a
  claim; the next tester tests it.
- **Cap reached with defects outstanding → `CAPPED`.** Not a pass. Name what's open.
- **Thrash → `ESCALATED`.** Stop and hand it to the user. The three shapes:
  1. **A defect is fixed and reappears in a later round** — the fix didn't hold, or
     tester and fixer disagree about correct. The tester's `RE-RAISED:` line against a
     defect the ledger marks *fixed* is the tell.
  2. **Two rounds' fixes contradict each other** — the loop is oscillating.
  3. **Defects per round stops falling** while the same surface keeps failing — a design
     problem the loop can only keep patching.

  Also escalate when a round **couldn't drive the app at all** (sign-in, environment,
  data) — the tester's summary will say so. That is not a clean round and must never be
  counted as one.

---

## Phase 3: Report

Compose the report from the round summaries you already hold — do **not** read the run
directory back to write it. The report is a summary by design; it points at the run
directory for the evidence.

```markdown
# Adversarial QE — <target>

**Verdict:** <CONVERGED | CAPPED | ESCALATED>
**Rounds:** <n> of <cap>  ·  **Lenses:** <round → lens, …>
**Exercised:** <routes/URLs> as <roles>  ·  **Data:** <tenant/practice, populated + empty availability>
**Test gate:** <command> — <result> (baseline: <result from the scout>)
**Run directory:** <path> — full defects, fixes, and ledger

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
request review — the caller (`/execute-brd`, or the user) owns what comes next. Leave the
run directory in place; it is temporary storage the user may want to read.

---

## Standards & guardrails

- **The empty round has to be real.** A `CONVERGED` reached by downgrading defects, by a round that never reached the page, or by testing only the path the fix was written for is a false pass — and a false pass here is worse than no run, because it is the last gate before a human.
- **A fresh subagent per round, and the tester is never the fixer.** Continuing the previous round's tester gets you the previous round's conclusions.
- **Orchestrate, don't work.** You dispatch and decide. Driving the browser yourself is what makes long runs impossible — and it is the most expensive thing this skill does.
- **Match the instrument to the question: snapshots for structure and behaviour, vision for what the user actually sees.** Looking is often the whole point of a round; what exhausts a Tidewave quota is the reflexive capture — per-action screenshots and polling loops — not the justified one.
- **State crosses on disk, not in context.** Subagents get the path to `handoff.md`, never its contents; they return the short contract, never prose.
- **Report only what the app did.** Every defect carries a reproducible click-path and an observed-vs-expected. No reasoning about what the code probably does — that is the code loop's job.
- **Expectations come from the BRD / ticket / replaced surface, and are written down before the app is touched.** Otherwise the run ratifies whatever it finds.
- **Verify each fix by re-driving the failing path**, and say so in the ledger.
- **Ask when the harness blocks you** after one or two attempts. A round that couldn't drive the app is never a clean round.
- **Bound the loop.** Cap the rounds, watch for thrash, escalate rather than grind.
- **Automation repos and `data-testid` names are QE's.** Describe, don't author. Never open a PR in an automation repo.
- **No PHI, secrets, or real patient/customer data** in a subagent prompt, the run directory, the report, or any browser script. Report record ids, counts, states, and timestamps — never names, DOB, MRN, contact details, or clinical free text, and never dump a whole record.
