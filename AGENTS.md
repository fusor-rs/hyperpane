# AGENTS.md

Rules for coding agents in this repository. They are not suggestions: a change
that breaks one is not finished, however well it works. When a rule and the task
truly conflict, stop and say so instead of quietly bending either.

hyperpane runs fusor applications as iPhone, iPad and Mac apps. Its host shows
the output of `fusor build` in WKWebView. The README and `docs/` describe what
it does; don't invent scope, features or options beyond the task.

## Repository

- `crates/hyperpane-shell`: the host. A library (publication checks, asset
  source, loopback listener, bridge, window and web view) and the
  `hyperpane-host` binary, which runs in a window on macOS and as the
  executable of an iOS app.
- `crates/hyperpane-cli`: the `hyperpane` binary (`run`, `bundle`, `doctor`,
  `upgrade`).
  It builds the application with fusor and the host with Cargo, assembles iOS
  bundles and drives the simulator with `xcrun simctl`. Each command is one
  file in `src/cmd/` holding its arguments, its `run` and its logic; a part
  that grows large becomes a child module (`cmd/run/simulator.rs`), and a
  command reuses another's logic from there (`run --ios` calls `bundle`).
  `cmd/mod.rs` declares the commands and the options they share. Outside
  `cmd/` are only what several commands use: building the application
  (`project.rs`) and the host (`host.rs`), output (`report.rs`) and running
  tools (`tool.rs`).
- `tests/consumer`: a separate workspace holding a small fusor application.
  `crates/hyperpane-cli/tests/consumer.rs` runs it through the CLI on macOS and
  in the iOS Simulator.
- `docs/`: the host's output contract (`consumption.md`) and the iOS bundle
  (`ios.md`).
- `install.sh` and `.github/workflows/release.yml`: the release archive and its
  installer. `just archive` builds the archive, `just archive-check` installs
  and checks it, and `just upgrade-check` checks `hyperpane upgrade` between two
  builds; the workflow runs all three.

Constraints that hold everywhere:

- Rust 1.85 (edition 2024) is the minimum; don't use newer language or library
  features. Unsafe code is forbidden: tao and wry own the Objective-C calls.
- One host for both platforms. Platform differences live behind
  `cfg(target_os)` in the smallest region that needs them: today, where the run
  configuration comes from.
- hyperpane consumes what `fusor build` publishes and adds nothing to fusor.
  fusor crates are not dependencies of the shell or the CLI; the consumer takes
  them from the sibling fusor checkout (see `CONTRIBUTING.md`).
- A gap in fusor gets an issue in fusor-rs/fusor and the narrowest workaround
  here, marked by a one-line comment carrying the issue URL. Never copy fusor
  code into this repository and never fix it in the fusor checkout.
- Every dependency is pinned to an exact version and must build on Rust 1.85.
- Every crate shares the version in `[workspace.package]`.

## The standard

Write the smallest correct change that a careful senior reviewer would merge
without comments. Agent-written code tends to be more verbose, more duplicated
and more eroded than human code. Assume your first draft has those faults and
remove them before you finish.

- **Less code, never compressed code.** Between two correct, equally readable
  solutions, take the one with less logic. Deleting code is progress. Report
  the net line change of your diff; every added line must be needed by the
  task. Squeezing the same logic into fewer, denser lines is worse code. When
  brevity and readability conflict, readability wins.
- **Write for the maintainer, not the grader.** Passing the checks is the
  minimum. The code must be easy to read, extend and debug by someone who
  never saw the task.
- **Read before writing.** Before adding a function, type, constant or
  dependency, search for one that already does it (`rg -n '<name or concept>'`)
  and use or extend it.
- **Verify, don't assume.** Check every API you call against its source
  (`~/.cargo/registry/src/*/<crate>-<version>/`), in particular wry's and tao's
  per-platform code: much of wry is macOS-only or iOS-only. Never call a method
  or add a crate you have not confirmed exists.
- **Run it.** "It compiles" is not evidence it works. Execute the behavior you
  changed on the platform it affects, macOS window or iOS Simulator, and say
  what you ran.
- **Fix causes, not symptoms.** Reproduce a bug first. Don't add a check,
  retry, sleep or fallback to make a failure disappear without knowing why.

## Scope and integrity

- **Do exactly the task.** Change the smallest region that achieves it. Don't
  refactor, rename, reformat or "modernize" code the task doesn't need.
- **Don't break what's next to it.** Before editing anything shared, find every
  other user with `rg` and confirm they still behave the same. Then run the
  full checks.
- **Finish or say so.** Never present partial, stubbed or untested work as
  done. If you can't complete the task, state exactly what is missing.
- **Don't expand the task.** Problems you noticed outside the task go in one
  line of your summary, unfixed.
- **Never game a check.** Don't special-case test inputs, hardcode expected
  outputs, or delete, skip, `#[ignore]`, loosen or rewrite a test, assertion or
  fixture to make it pass. A simulator failure is not a reason to weaken the
  macOS test, or the reverse.
- **Never fake the result.** No canned data in place of real computation, no
  stand-in in a production path, no output staged to resemble success.
- **Never destroy what you didn't create.** No deleting files or data outside
  the task, and no `rm -rf`, `git reset --hard`, `git clean`, `git checkout --`
  on uncommitted work or force pushes without explicit permission. The CLI
  replaces only app bundles it assembled itself; keep it that way.
- **Match the repository.** Follow the structure, naming and idioms already
  here.

## Hard rules

### Duplication and abstraction

- No near-duplicate blocks. Code that repeats with small edits becomes one
  function, loop or table.
- One concept, one implementation. There is one asset responder for the custom
  scheme and the loopback listener, one error type with a remedy, one way to run
  an external tool (`tool::capture`). Extend them; don't add a second path.
- One source of truth per constant: file names in a bundle, the minimum iOS,
  pinned versions. Reference the constant in logic and in messages.
- No speculative generality: no trait with one implementation, generic with one
  instantiation, option or parameter without a caller that needs it today.
- No trivial wrappers, and no `utils`, `helpers`, `common` or `misc` modules or
  `Manager`, `Helper`, `Util`, `Data`, `Info`, `Impl` or `Wrapper` type names.

### Readability

- One statement per line, formatted by `rustfmt` with default settings.
- Names are full words from the domain. Single letters only for loop counters
  and coordinates.
- No magic numbers or strings in logic. Give them a named constant.
- Long text never sits in one-line literals. Scripts injected into the page are
  their own `.js` files (`include_str!`); the `Info.plist` template is a
  multi-line literal laid out as a plist.
- Propagate errors with `?`. Check a repeated guard once.
- Decompose by concept. A function reads as one level of abstraction; a file
  holds one concept.

### Functions and types

- Functions stay under 60 lines, with at most 5 parameters and at most 3
  levels of nested blocks. Flatten with early returns and `let … else`.
- No boolean parameters that switch behavior. Use two functions or an enum.
- Make invalid states unrepresentable: enums instead of string tags, no
  `Option<Option<T>>`, no paired `Option`s.
- Default visibility is private, then `pub(crate)`. `pub` only for the API the
  CLI uses from the shell.

### Errors

- Never swallow a failure. No `.ok()`, `unwrap_or_default()`, `unwrap_or(…)`,
  `let _ = …` or empty arm on a `Result` unless absence is a real, expected
  outcome, stated in a one-line comment.
- No `unwrap()` outside tests. `expect("…")` only for invariants the code
  guarantees; the message states the invariant.
- Errors are `hyperpane_shell::Error`: a kind, what failed, and the remedy, an
  exact command where one exists.
- The host's standard output is its contract: one JSON line per event in
  `docs/consumption.md`. Nothing else goes to the host's standard output, and
  the CLI prints only through `report.rs`.

### Comments: default zero

A comment is allowed only when deleting it would let a competent reader make a
wrong change: an invariant or ordering requirement (the window is created on
`StartCause::Init`); a constraint not visible in the code, with a link (a wry
limitation, a fusor issue); why an obvious alternative was not used. No
narration, banners, commented-out code, doc comments that repeat a signature,
process comments ("now", "updated", "fixed") or `TODO`s.

Docs describe current behavior plainly. No work logs, changelog tense or hedged
non-claims.

### Dependencies

- Prefer the standard library. Add a dependency only for functionality the task
  needs, pin it exactly, confirm its API and its `rust-version`, and say why in
  the pull request.
- No `async`, `Arc`, `Mutex` or threads unless the task needs concurrency; the
  loopback listener is the one place that does.

### Tests: only what guards behavior

- Write the fewest tests that would catch a real regression of the behavior you
  changed. Table-driven when the behavior has variations.
- Web view behavior belongs in the consumer test, which runs real WebKit on
  both platforms; extend its probe and assertions rather than adding a parallel
  harness.
- Expected values are literals worked out independently, never computed by
  the code under test.
- Every new test must fail without the change it covers. Check it.
- No sleeps or wall-clock timing in Rust tests. The probe waits for observable
  page state with a deadline.

### Suppressions

- Never silence the compiler or Clippy with `#[allow(…)]`. A justified
  exception is `#[expect(lint, reason = "…")]`.
- fusor's generated template code is outside this repository: wrap only the
  `template!` call in a module carrying `#[expect]`, link the fusor issue, and
  never edit generated output.

## Enforced lints

`[workspace.lints]` in the root `Cargo.toml` and `clippy.toml` are the lint
configuration; each crate opts in with `[lints] workspace = true`.
`tests/consumer` is a separate workspace and repeats the tables, so change them
together. Clippy denies, among others, `unwrap_used`, `allow_attributes`,
`too_many_lines` (60), `cognitive_complexity` (15), `excessive_nesting`,
`min_ident_chars`, `wildcard_imports`, `needless_pass_by_value` and
`redundant_clone`. `just lint` also runs Clippy for the iOS Simulator target, so
code behind `cfg(target_os = "ios")` is checked.

## Checks

A change is done only when this passes on a Mac with Xcode:

```sh
just check
```

It runs formatting, the workspace tests, Clippy for macOS and the iOS
Simulator, a check of the iOS device target, rustdoc, the Rust 1.85 build, and
the consumer test on macOS and in the iOS Simulator. `just doctor` explains a
missing tool.

## Final review

Before reporting a task as done, review your own diff, not your memory of it:

```sh
git diff --stat
git diff | grep -nE '^\+.*(unwrap\(\)|\.ok\(\)|unwrap_or_default\(\)|let _ =|#\[allow|todo!|unimplemented!|dbg!|TODO|FIXME)'
git diff | grep -niE '^\+\s*//.*\b(now|updated?|fixed|previously|new:|step [0-9]|as requested)\b'
git diff | grep -nE '^\+\s*(//|/\*)'
git diff | grep -nE '^\+\s*#\[test\]'
git diff | grep -nE '^\+.{101,}'
```

Justify or remove every hit. Then confirm:

1. Nothing duplicates code that already existed, inside or outside the diff.
2. Every new function, type, parameter and dependency has a current caller.
3. Every behavior change was executed or is covered by a test that fails
   without it.
4. The diff contains nothing outside the task.
5. Each changed function reads top to bottom with no dead code or leftovers.
6. The change builds what the task asked for, not something that only looks
   like it.

## Reporting

Say what changed, what you ran on which platform and what it showed, the net
line change, and anything you could not verify. Report failures and skipped
checks plainly.

## Commits

- Commit messages are a short imperative sentence in sentence case, no prefix.
- Never add `Co-Authored-By`, "Generated with" or any other AI attribution to
  commits or pull requests.
- Don't commit, push, tag or release unless asked. Releasing is described in
  `CONTRIBUTING.md` and is done by the maintainer.
