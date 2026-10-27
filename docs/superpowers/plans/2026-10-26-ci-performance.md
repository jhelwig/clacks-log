# Windows continuous integration performance implementation plan

## Goal and constraints

Reduce continuous integration (CI) compilation/linking and test execution time
on Windows and compare Linux
against the same successful baseline, run 40440079456 (commit 703b566). Preserve
Rust 1.99.1, production SQLite durability settings, all existing check categories,
PR/main trigger scope, and real Clacks behavior assertions. Test count and line
coverage are not goals. Keep audit findings and measured run evidence in the PR,
not a repository review report. This is a performance-focused test review, not a
claim that every remaining test in the repository has been audited.

## Baseline and approach

The baseline Windows job took 18m33s: seven minutes compiling workspace tests,
roughly 5m40s executing them, and the remainder in startup, dependency setup and
other checks. Its dependency-cache restore was an exact hit. The desktop suite
alone ran for 224 seconds. Linux took nine minutes.

Prefer reducing redundant work to merely retaining larger build directories:
consolidate integration suites where separate executables repeatedly compile
shared fixtures and link the same dependency graph, and remove unjustified test
work. Workspace-artifact caching alone does not establish reuse across fresh
checkouts. Retain full checks and measure the resulting run. Use compiler profile
or linker changes only if measurements justify them; preserve useful diagnostics.

## Tasks

1. Measure the unchanged workspace build with dependencies retained and own
   artifacts cleaned, incremental compilation disabled as in CI, and Cargo's
   stable `--timings` report. Inspect expensive test fixtures and assertions,
   including the production requirements they claim to verify. Save measurements
   outside the repository.
2. Give CI separate workspace compilation and execution steps using
   `cargo build --workspace --all-targets --timings` followed by the
   existing test command. Run the built desktop executable directly for its CLI
   smoke check, avoiding another feature-subset compilation. Preserve timing HTML as a short-lived CI artifact on
   both platforms, including when another check fails. Keep SQL metadata scoped
   to client-store and retain formatting, CLI startup, doctests, Clippy and
   rustdoc checks.
3. Consolidate the model, sync, client-store and LoTW integration tests into
   explicit suite targets with shared fixture modules compiled once. Keep test
   modules separated by responsibility, preserve actual scenarios, update exact
   child-process test names where required, and verify test discovery. Do not
   add a test dispatcher, public test-only API or dependency.
4. Remove the redundant store `Send`-bound runtime test after verifying the real
   production task paths enforce the same bound. Correct the main-screen test's
   irrelevant scrolling/frame sweep while retaining its appearance/scale/size
   scenarios. Bound the operator-dialog layout scenarios to meaningful geometry
   extremes, including logbook form reachability, and make every retained Apply
   actually change the operator. Preserve
   keyboard, validation and genuine commit/cancel coverage.
5. Compare the own-crate rebuild with the same local baseline conditions. Run
   discovery, all workspace tests, doctests, formatting, Clippy, rustdoc and SQL
   metadata checks appropriate to changed files. Review changed tests separately
   for a concrete Clacks requirement and mistake they detect. Do not replace
   removed weak tests with tests of framework behavior.
6. Commit coherent changes with reviewed, physically wrapped messages. Push to
   PR #53, monitor Windows and Linux, diagnose failures, and compare total,
   compilation and execution times against the baseline. Distinguish cache
   population from steady-state results. Record the comparison and audit scope
   in the PR and delivery response. Leave the worktree for review.
