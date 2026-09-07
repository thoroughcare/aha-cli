---
name: audit-docs
description: "Audit a repo's documentation — the docs/ folder, root markdown
  files (CLAUDE.md, AGENTS.md, README.md, GLOSSARY.md, and any sub-directory
  AGENTS.md), and skill definition files (`.claude/skills/` or
  `plugins/*/skills/` SKILL.md) — for accuracy against the actual codebase, and
  streamline them by removing redundancy and duplication. Reports drift (stale
  references to renamed/removed code, acceptance criteria that no longer match
  behavior, guide steps with outdated paths, skills pointing at moved files) AND
  duplication (same fact restated across files, sections that could reference a
  single source of truth, padding that adds no information). Read-only by
  default — proposes edits, waits for user approval before applying. TRIGGER
  when: user asks to audit docs, verify docs/user-stories are accurate, \"are
  the X docs still right\", \"check that AGENTS.md matches the code\",
  \"validate the skills still work\", \"deduplicate the X docs\", \"tighten the
  docs for Y\", or types /docs:audit-docs [scope-or-instructions]. SKIP when:
  the user is adding brand-new docs for a feature they just built (do that
  inline as part of the PR rather than as an audit pass)."
---

# Audit Docs

Verify that documentation accurately describes the current codebase AND that it
is not redundant or duplicated across files. Reports drift and duplication,
proposes fixes, waits for approval before editing. Pure docs work; never touches
application source, library, config, or test code.

$ARGUMENTS

---

## Phase 1: Parse the argument

The argument can be a **scope**, an **instruction**, or both. Decide which before
proceeding.

- **Scope** — a path, folder, file, skill name, or topic. Examples:
  `docs/user-stories/`, `AGENTS.md`, "authentication flow".
- **Instruction** — explicit directive about what to look for or how to act.
  Examples: "deduplicate", "tighten — too verbose", "only check Source: lines",
  "merge anything that overlaps between a.md and b.md".
- **Both** — e.g., "audit the auth docs and merge duplicates with the session
  docs".

If no argument is given, ask the user to pick a scope before proceeding — the
full docs tree is usually too large to audit in one pass. Suggest:
- A bounded folder (e.g., `docs/user-stories/<area>/`)
- A single doc file (e.g., `docs/guides/<topic>.md`)
- A single skill (e.g., `.claude/skills/<name>/SKILL.md` or `plugins/<name>/skills/<name>/SKILL.md`)
- A root file (`CLAUDE.md`, `AGENTS.md`, `README.md`, `GLOSSARY.md`)
- A topic ("authentication", "background jobs")

If the argument is a topic rather than a path, map it to the matching folder
using any index the repo keeps (e.g., a `docs/README.md` table of contents)
before continuing.

If the argument contains explicit instructions, treat them as directives that
override the defaults in Phases 2–4 (e.g., "only check links" → skip the code
identifier checks).

---

## Phase 2: Identify the in-scope doc files

List the markdown files to audit based on the scope:
- Topic argument → the folder(s) under `docs/` that cover it, plus related guides
- Path argument → that file/folder only
- Skill argument → that `SKILL.md` plus anything it references

Read each in full. Record the testable claims:
- `Source: path/...` pointers (must resolve)
- Code identifiers (class/module names, methods, constants, flag names, route
  paths, config keys)
- Step-by-step procedures (file paths, command names, config keys)
- Role names, status enums, other domain vocabulary
- Cross-doc links (`[label](other.md)`)

Also record the **prose content** of each section so Phase 3 can compare across
files for duplication.

---

## Phase 3: Check for accuracy AND redundancy

### 3a. Accuracy — verify each claim against the codebase

For every recorded claim, run the cheapest check that proves it true or false.
Adapt the exact commands to the repo's language and layout:

| Claim type | Check |
|---|---|
| `Source: path/to/file` | confirm that path exists (`ls` / file search) |
| Class / module / type name | search the source tree for its definition |
| Method / function name | search the file the doc points at for its definition |
| Constant / enum value | search the source tree for the symbol |
| Config / flag name | confirm the config entry or flag definition exists |
| Route / endpoint path | search the routing/config layer for the path |
| Cross-doc link | confirm the linked file resolves from the doc's directory |
| Skill referencing another skill / shared step | confirm the target file/section exists |

Accuracy categories:
- **Broken** — references something that no longer exists. Must fix.
- **Stale** — references the old name/path of something that was renamed. Must fix.
- **Drifted** — acceptance criterion no longer matches observable behavior. Must fix.
- **Missing** — code has user-facing capability with no story/doc (only flag for
  user-stories-style scope; do not chase exhaustively).
- **Correct** — no change.

### 3b. Redundancy — find duplication and bloat

Compare sections across the in-scope files (and against the canonical source of
truth for that topic) to identify:

- **Duplicated facts** — same claim stated in two or more places. Pick one as
  authoritative per the repo's documentation policy ("single source of truth"),
  have the other reference it.
- **Restated procedure** — a step list reproduced in a skill that another skill (or
  an installed marketplace plugin) already owns. Replace with a reference to it.
- **Summary-plus-full-text pairs** — a rule stated in brief in `AGENTS.md`/`CLAUDE.md`
  *and* in full in a second doc the summary points at. A reader can't tell which is
  authoritative and the two drift. Collapse to one home: a rule belongs in the agent
  docs, an ordered procedure in the skill that performs it.
- **Padding** — sentences that don't add information (filler, hedging, restating
  the section header in prose, "this section will cover..."). Remove.
- **Stale alternatives** — multiple how-to paths described where one is now
  deprecated. Either collapse to the current path or label the legacy one clearly.
- **Over-specified prose** — paragraphs that re-derive what a code identifier
  already names. Trim.

Redundancy categories:
- **Duplicate** — same fact in 2+ files; collapse to one + reference.
- **Bloat** — prose that can be cut with no information loss.
- **Legacy-mixed** — current and deprecated paths described side-by-side without
  labeling.

### 3c. Streamlining guardrail

Before proposing any cut, confirm the content is genuinely redundant or bloat —
not a fact that just happens to also appear elsewhere in a different framing.
When in doubt, keep it — the cost of losing a relevant detail is higher than the
cost of leaving a redundant sentence.

Specifically, never cut:
- The only place a constraint, gotcha, or "why" is recorded.
- Acceptance criteria, even if they feel verbose — they are the contract.
- Source pointers (`Source: ...`).
- Cross-links to adjacent contexts.
- Anything the user explicitly flagged as load-bearing in the argument.

---

## Phase 4: Report findings

Present a numbered list to the user, grouped by file, with accuracy and
redundancy findings interleaved:

```
docs/user-stories/<area>/problems.md
  1. BROKEN — "Source: app/models/old_name" — renamed to app/models/new_name
  2. DRIFTED — AC says "max 200 chars" but the model validates length: 500
  3. DUPLICATE — this story is also in <other>.md (same AC, same Source). Keep
     here, replace the other with "See also:".
  4. BLOAT — opening sentence just restates the section header.

.claude/skills/<name>/SKILL.md
  5. LEGACY-MIXED — describes both the current and an older removed pattern with
     no note that the older one was removed.
```

Each finding shows: file, category, what's wrong, evidence. No edits yet.

---

## Phase 5: Apply fixes with approval

Ask the user which findings to fix. For each approved item:
- Use `Edit` (not `Write`) — minimal, targeted changes.
- For `Source:` line fixes, update the path only (omit line numbers if the repo's
  convention is paths-only, since line numbers rot with churn).
- For `Drifted` acceptance criteria, propose the corrected wording and confirm
  before editing.
- For `Duplicate` findings, propose which copy to keep and confirm before
  collapsing.
- For `Missing` stories, don't auto-write them — flag them for the user to add in
  the relevant feature's PR, where the feature owner has the context the doc needs.

Follow the repo's staging policy — if it asks you not to run `git add`, leave the
changes for the user to stage.

---

## Phase 6: Wrap up

Report what was changed (file:line list), what was left unfixed and why, and
any follow-up work that would belong in a separate PR (e.g., missing docs that
should land with the feature owner, or a deduplication that touches more files
than the requested scope).

---

## Reminders

- **Codebase wins.** If a doc says X and the code does Y, the doc is wrong —
  not the other way round. The exception is when the user says the code is
  broken and the doc describes the intended behavior.
- **Single source of truth.** When two docs say the same thing, pick one as
  authoritative and have the others reference it. Don't duplicate.
- **Streamline, don't strip.** Cut bloat and duplication; never cut a fact,
  constraint, gotcha, acceptance criterion, or Source pointer. When in doubt,
  keep it.
- **Scope-aware.** A user asking about one area doesn't want a full-repo doc
  audit. Stay inside the requested area unless a duplicate spans into an
  adjacent context, in which case flag it but don't edit outside scope without
  asking.
- **Read first, edit second.** Always show findings before changing anything.
- **For tech-stack version drift, this skill ignores version numbers in docs** —
  it checks identifiers, paths, and behavior, not dependency versions.
