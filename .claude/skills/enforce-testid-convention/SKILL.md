---
name: enforce-testid-convention
description: 'Apply a consistent data-testid naming convention when adding or
  auditing UI markup across template engines (Vue SFCs, HEEx/LiveView templates,
  and ERB views). Generates kebab-case testid values matching
  [feature]-[component]-[element]-[action|state] (the default — override per
  repo), flags violations, and offers diffs. TRIGGER when: user asks to add
  testids to a component, audit testids, "follow the data-testid standard for
  X", or types /testid-convention:enforce-testid-convention <path>. SKIP when:
  user is only renaming an existing testid (do that inline) or the file is not a
  UI template (Vue SFC, HEEx/LiveView template, or ERB view).'
---

# Enforce data-testid Convention

Apply the project `data-testid` standard to a UI template. Works across the three
template surfaces a project might use:

- **Vue SFCs** (`.vue`) — `:data-testid` bindings for dynamic values.
- **HEEx / Phoenix LiveView** (`.heex`, `~H` sigils in `.ex`) — `{...}` interpolation in HEEx.
- **ERB views** (`.html.erb`) — `<%= ... %>` interpolation.

Two modes:

1. **Author mode** — adding interactive UI elements: propose a correctly-formatted `data-testid` for each one.
2. **Audit mode** — given an existing file: list every interactive element missing a testid (or violating the convention) and propose values. Present the audit as a table so the gaps and fixes are scannable:

   | element | location | current | proposed | reason |
   |---|---|---|---|---|
   | Save button | `PatientRiskLevel.vue:42` | — | `patient-risk-level-save` | missing testid |
   | Cancel button | `PatientRiskLevel.vue:48` | `cancelBtn` | `patient-risk-level-modal-cancel` | camelCase → kebab |

Argument: a file path or component name (e.g. `components/features/patient/PatientRiskLevel.vue`,
`lib/app_web/live/patient_live/index.ex`, or `app/views/admin/practices/_form.html.erb`).

$ARGUMENTS

If no argument is provided, ask the user which file to audit.

> **Cross-repo standard.** This `data-testid` convention is shared across repos — the
> same value identifies the same logical element regardless of template engine. When the
> same UI is being ported from one stack to another (e.g. Rails/Vue → Elixir/LiveView),
> **reuse the existing `data-testid` value** so QE's existing Playwright tests keep
> passing. Grep the other repo for the equivalent element before inventing a new value.

> **Naming ownership.** In some teams `data-testid` naming is owned by QE, not the dev
> team. If the project documents that (e.g. in AGENTS.md/CLAUDE.md), **don't invent a new
> value when one doesn't already exist — surface the gap and ask** rather than guessing.

---

## Phase 0 — Discover the convention

Before proposing values, confirm the project's actual convention. Check, in order:

1. The repo's `AGENTS.md` / `CLAUDE.md` / docs for a documented `data-testid` standard.
2. Existing `data-testid` values in the codebase (grep) to infer the pattern in use.
3. Fall back to the default below only if nothing is documented or established.

**Default pattern** (override with whatever the repo documents):

- **Format:** kebab-case, `[a-z0-9-]` only — no underscores, no camelCase, no spaces.
- **Pattern:** `[feature]-[component]-[element]-[action|state]` — omit unused segments.
- **Dynamic suffix:** only when the element appears in a list — `...-${id}` / `...-#{id}` / `...-<%= id %>`.
- **Semantic intent:** describe what the element *is for*, not where it sits in the DOM or how it's styled.
- **Attribute name:** always `data-testid` — QE selectors match on that exact string, so `data-test-id`, `test-id`, and `testid` won't be found. Reject them.

---

## Phase 1 — Identify candidate elements

Interactive or test-relevant elements that should carry a testid:

- `<button>`, `<input>`, `<select>`, `<textarea>`
- Form submit buttons and primary CTAs
- Modal / dialog roots and their open/close triggers
- Page roots and primary sections (top-level `<section>` / `<main>` / outermost element of a template)
- Tables and primary rows (id-bearing rows use a dynamic suffix), key data cells
- Navigation tabs (tab triggers and tab panels), status badges

**Skip** pure-presentational elements (`<span>`, layout-only `<div>` wrappers) **unless** they
are the only stable hook QE has for a region.

---

## Phase 2 — Propose values

Derive each segment from the file's context. The sources differ by template engine:

| Segment | Vue SFC | HEEx / LiveView | ERB |
|---|---|---|---|
| `[feature]` | Owning folder under `components/features/` (e.g. `features/patient/flags` → `patient-flags`). | LiveView namespace / folder (e.g. `live/patient/flags_live` → `patient-flags`). | Controller namespace / view folder (e.g. `views/admin/practices/` → `admin-practices`). |
| `[component]` | SFC name in kebab (`PatientRiskLevel.vue` → `patient-risk-level`). | LiveView/component module in kebab (`PatientRiskLive` → `patient-risk`). | Template/partial name (`_form.html.erb` → `form`). |
| `[element]` | Semantic role (`button`, `input`, `modal`, `table`, `row`, `tab`). | Same. | Same. |
| `[action\|state]` | Handler or label (`save`, `cancel`, `open`, `close`, `expanded`, `selected`). | Same (often the `phx-click` event name). | Same. |

Examples:
- Save button in `PatientRiskLevel.vue` → `patient-risk-level-save`
- Edit icon → `patient-risk-level-edit`
- Cancel button in a modal → `patient-risk-level-modal-cancel`

---

## Phase 3 — Apply / present diff

Use `Edit` to add the attribute next to existing attributes (don't reorder). Show the diff
before committing. Dynamic-value syntax differs by template engine:

- **Vue SFC** — use the binding form `:data-testid="`patient-risk-level-row-${row.id}`"` (Vue
  interprets the value as a JS expression). A static `data-testid="${foo}"` would emit the
  literal `${foo}` string at runtime.
- **HEEx / LiveView** — interpolate with `{...}`:
  `data-testid={"patient-risk-level-row-#{row.id}"}`. Keep dynamic segments kebab-case; if a
  value can contain underscores or mixed case, normalize it (e.g.
  `String.replace("_", "-")` / a slug helper) so it doesn't violate the convention.
- **ERB** — interpolate inside the attribute value:
  `data-testid="<%= "patient-risk-level-row-#{row.id}" %>"`. Use `.to_s.dasherize` /
  `.parameterize` so dynamic segments stay kebab-case (a Ruby symbol `:reading_target`
  interpolates as `reading_target`, which would violate the convention without normalization).

---

## Reminders

- **Keep values kebab-case** — QE tooling assumes it, so re-derive in kebab if camelCase or snake_case slipped through.
- **Prefer the leaf semantic element** (the `<button>`, the `<input>`) — not its wrapper.
- **For repeated elements** (Vue `v-for`, HEEx `:for`, ERB `.each`), suffix with the record id
  when stable, else the index.
- **Reuse existing values across stacks.** When porting UI between repos, keep the same
  `data-testid` so QE's existing tests keep passing.
- **Flag, don't silently rename.** Flag any non-conforming values the dev already wrote — but
  **do NOT rename them** in this pass (renaming an existing testid is a separate, inline change
  that can break QE tests asserting the old value).
