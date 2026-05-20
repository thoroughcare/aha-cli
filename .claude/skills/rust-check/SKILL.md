---
name: rust-check
description: |
  Run the project's full Rust verification gate (fmt + clippy + tests) before
  declaring a change complete, and police project-specific Rust idioms in any
  code that was edited.
  TRIGGER when: about to claim a task done that touched `.rs` files, opening a
  PR, the user asks to "verify", "check", or "lint", or after a non-trivial
  edit in `src/` or `tests/`.
  SKIP when: the change is docs-only (`*.md`, `docs/`), the user explicitly
  scoped the request to a quick read, or the repo is in a known mid-refactor
  state the user flagged.
---

# Rust Check

Verify the project compiles cleanly, passes lint, and tests still pass.
Also enforce the project's Rust idioms on any code that was edited in this
session.

## Phase 1: Run the gate

Run all three. Do not skip clippy because "it's just a small change" — the
project treats warnings as errors in CI-equivalent flow.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

If `cargo fmt --check` fails: run `cargo fmt --all` and re-run the gate.
Don't hand-patch formatting.

If clippy fires: fix the underlying issue. Only `#[allow(...)]` with an
inline comment explaining why, and only if the lint is genuinely wrong for
the call site (e.g. the existing `#[allow(dead_code)]` on
`OneEnvelope<T>` in `src/client/resources.rs`).

## Phase 2: Idiom checks on edited code

Scan the files you touched against the broader Rust community's
conventions and **enforce** them — fix violations in the code you
edited rather than just noting them. These are the defaults most mature
Rust codebases converge on; the project benefits from holding the line
even when surrounding code has drifted.

When you fix a violation, do it in the smallest scope that makes the
edited code correct. Don't go on a repo-wide sweep refactoring code you
didn't otherwise touch — that's a separate task. If you encounter a
violation in unrelated code that would take more than a trivial fix,
surface it to the user and let them decide whether to expand scope.

### Error handling
- No `.unwrap()` or `.expect()` in non-test code paths that handle
  fallible IO, parsing, or user input. Reserve them for invariants the
  compiler can't see, and when you do use `.expect("...")`, the message
  should describe *why* the invariant holds, not restate the type.
- Don't use `panic!`, `unreachable!`, or `todo!` on a path a user can
  reach. `unreachable!` is fine for an exhausted match in control flow
  the compiler can't prove; `todo!` should never ship.
- Return `Result`, don't swallow with `let _ = ...` unless you've
  thought about it. Annotate `#[must_use]` on result-bearing helpers
  where dropping the value would be a bug.
- Errors should preserve context. Whether the project uses `anyhow`,
  `thiserror`, `eyre`, or a hand-rolled enum, every `?` boundary should
  add information (`.context("...")` / `.map_err(...)`) when the lower
  error wouldn't be self-explanatory at the call site.

### Ownership and borrowing
- Prefer `&str` over `&String` and `&[T]` over `&Vec<T>` in function
  signatures. Take `impl AsRef<Path>` / `impl Into<String>` when the
  call site is otherwise forced to clone.
- Don't `.clone()` to silence the borrow checker without considering
  whether the lifetime could be restructured. Cloning small `Copy`-ish
  values is fine; cloning `Vec`/`String` in hot paths is a smell.
- Reach for `Cow<'_, str>` when a value is *usually* borrowed but
  *sometimes* owned, instead of forcing an allocation on every path.

### API design
- Public items get rustdoc. Use `///` with a one-line summary, blank
  line, then details. Examples in doc comments are tested by
  `cargo test --doc` — make sure they compile or mark them
  `no_run` / `ignore` with a reason.
- Builders / constructors with many optional fields use the typestate
  or builder pattern, not a 12-argument `new`.
- Prefer `&self` over `&mut self` where possible — interior mutability
  via `Cell`/`RefCell`/`Mutex` is sometimes the right call for
  ergonomic public APIs.
- Newtypes (`struct UserId(String);`) carry meaning the bare type
  doesn't. Use them for IDs, units, and any value with semantic
  constraints.

### Async
- Don't block in async code. `std::thread::sleep`, `std::fs::*`, and
  synchronous `reqwest::blocking` inside a `tokio` task will stall the
  runtime. Use the async equivalents.
- `tokio::spawn` is fire-and-forget — make sure either the joinhandle
  is awaited or the work is genuinely independent. Bound parallel
  fan-out (e.g. `futures::stream::StreamExt::buffer_unordered`) when
  hitting a rate-limited resource.
- Cancellation safety matters in `select!`. Read each branch as "what
  partially-completed state could this leave behind?"

### Naming and structure
- Snake_case for functions, variables, modules; UpperCamelCase for
  types and traits; SCREAMING_SNAKE_CASE for consts and statics.
- Module names are singular nouns (`client`, not `clients`) unless
  the module genuinely represents a collection.
- Keep `mod.rs` / `lib.rs` thin — they re-export and wire up
  submodules; logic belongs in named files.

### Comments
- Default to writing no comments. Add one only when *why* is
  non-obvious — a hidden constraint, a workaround for a specific bug,
  behavior that would surprise a reader.
- Module-level `//!` comments are a good place for invariants the
  module assumes. Item-level `///` doc comments describe contract,
  not implementation.
- Don't leave `// TODO`, `// FIXME`, or `// removed X` markers in a
  diff you're about to ship. Open an issue or delete.

### Tests
- Co-locate unit tests in `#[cfg(test)] mod tests { ... }` at the
  bottom of the file under test. Use `tests/` for integration tests
  that exercise the crate's public surface.
- Each test name describes the behavior under test, not the function
  being called: `returns_none_when_empty`, not `test_get`.
- Prefer table-driven tests over copy-pasted near-duplicates.
- Snapshot tests (`insta`, `expect-test`) are great for stable output
  shapes — review pending snapshots, never accept blind.

## Phase 3: Report back

- If the gate passed and idioms are clean: say so in one line and move on.
- If clippy or tests failed: paste the failing chunk, then fix.
- If you fixed idiom violations in your edited code: mention what you
  changed and why, so the user can review the scope of the fix.
- If you spotted an idiom violation in *unrelated* existing code that
  you didn't touch: mention it briefly so the user knows, but don't
  refactor it as part of this task.

## Reminders
- Never run with `--no-verify` or skip the gate to "save time" — broken
  builds in `main` are far more expensive than the 30s gate.
- `cargo test` runs both unit tests and the `tests/` integration suite.
  Don't substitute `cargo check`.
- If a test is genuinely flaky, surface it — don't loop until it passes.
