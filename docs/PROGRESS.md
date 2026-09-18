# Ferrite — Progress Log

> Append-only, dated, newest entry last. Started empty at the P0 rebuild
> (`docs/REBUILD_DIRECTIVE.md`) — for pre-rebuild history see
> `docs/archive/PROGRESS.md` (banner: historical, do not use as context).

## Format contract (every entry follows this shape — R2/R3)

```
## YYYY-MM-DD — <agent id> — <one-line summary>

**Landed:** what actually shipped, in plain language.
**Commits:** `<sha>` `<sha>` ... (one per green cycle, per R3)
**Tests:** `<test_name>` `<test_name>` ... — the tests that would fail if this were reverted (R1/R2)
**Known issues discovered:** anything found but not fixed here, with the T-### it was filed as (docs/TO-DO.md)
```

A claim with no cited test name or commit SHA does not belong in this file —
write "not implemented" instead (R2). This is the habit the old
`Browser/PROGRESS.md` (now archived) got right and the new format keeps.

---

## 2026-09-18 — A0 (Archivist) — repo flattened, dead architecture purged, doc layer reset

**Landed:**
- Flattened the workspace from `Browser/*` to the repo root via pure `git mv`
  (zero content edits), verified with `cargo metadata --no-deps` before/after
  showing an identical 7-member package set and identical per-package
  dependency/version/edition/feature signatures.
- Deleted `ferrite-capability-broker`, `ferrite-policy`, `ferrite-sandbox`
  (dormant, never workspace members) and `extensions/hello-ext` (their only
  consumer) — the dead capability-broker/Wasm-sandbox architecture per
  `docs/REBUILD_DIRECTIVE.md` §4.
- Deleted `.rules` and `commands.md` (described the dead architecture as
  current; `.rules` was the highest context-poisoning risk in the repo as an
  auto-ingested dotfile).
- Archived `EVALUATION_PLAN.md`, `FINALIZED_DECISIONS.md`,
  `PROJECT_REFERENCE.md`, `paper/`, `Resources/*.pdf`, and the old
  `PROGRESS.md`/`TO-DO.md` to `docs/archive/`, each with a `HISTORICAL —
  DESCRIBES A PROJECT THAT NO LONGER EXISTS` banner as its first line, plus
  `docs/archive/README.md` explaining why and a doc-drift check
  (`scripts/check_no_archive_links.sh`) that fails if any live doc links
  into the archive.
- Extracted the genuinely-true design decisions from `FINALIZED_DECISIONS.md`
  / `EVALUATION_PLAN.md` into `docs/DECISIONS.md` as ADR-000 through ADR-008,
  each citing the commit SHA where its implementation actually landed
  pre-rebuild, and each noting inline where the rebuild's defect register
  (D1–D14) identifies a gap between the decision and its old implementation.
- Rewrote `CLAUDE.md` to the zero-status contract (§12): invariants and
  pointers only, no "planned"/"not yet implemented" section.
- Rewrote `README.md` to a one-page "what it is today" description.
- Rewrote `.devcontainer/Dockerfile` and `.devcontainer/devcontainer.json`:
  removed the `ferrite-capability-broker` COPY/fetch (would have failed the
  image build against a now-deleted path), the `wasm32-unknown-unknown`
  target install (dead Wasm-sandbox target), and the `9222` port forward
  (dead JSON-RPC-agent leftover).
- Merged the root and `Browser/` `.gitignore`s into one at the repo root;
  narrowed the old blanket `*.txt` ignore to nothing (removed outright —
  no current need for it, and it silently swallowed any future `.txt`
  fixture).
- Regenerated `docs/TO-DO.md` as a stable `T-###` ledger derived from three
  sources: the D1–D14 defect register, the A1–A13 agent charters, and the
  still-open items in the old `TO-DO.md` (corpus-authoring design doc,
  per-case result inspector, schema-completeness audit, the Phase-6
  freeze-and-run hold, and the Extism/Wasm sandbox work — dropped, not
  silently, with reasons recorded).
- Installed the commit-msg guard hook (rejects AI attribution trailers) and
  a pre-commit `fmt`/`clippy`-on-staged-crates hook under `scripts/hooks/`,
  wired via `git config core.hooksPath`.
- Checked for live contributor branches before the flatten
  (`git branch -a --sort=-committerdate`): only `main` and one 68-day-stale
  Dependabot branch (`origin/dependabot/cargo/Browser/cargo-7c4a9e5e8f`,
  auto-generated, no human activity) — judged safe to proceed without
  further coordination.

Also fixed `.github/workflows/ci.yml`, which the flatten broke: every step
used `working-directory: Browser` / `Browser/**` path filters against a
directory that no longer existed, unconditionally installed a `wasm32`
target for the now-deleted extension sandbox, and had an unreachable
"Build hello-ext Wasm" step gated on a `linux` matrix entry the matrix
never had (windows/macos only) — that step referenced a directory this
same session deleted. Fixed in-session rather than deferred to A1, since
leaving CI broken from a change this session made is worse than touching a
file slightly outside A0's original scope.

Attempted to verify the devcontainer image actually builds
(`docker build -f .devcontainer/Dockerfile .`), not just that
`cargo metadata` resolves — a bad `COPY` path fails at image-build time,
which `cargo metadata` can't see. The Docker daemon isn't available in this
sandbox (CLI present, no running daemon, no Docker Desktop installed), so
the build genuinely failed (`connect: no such file or directory`) — the
background-task notification reported "completed, exit code 0" because the
wrapper script's own last command (an `echo`) succeeded even though the
`docker build` inside it didn't; caught by reading the actual log rather
than trusting the notification. Fell back to exhaustively verifying every
`COPY` source path and every stub-loop crate name in the rewritten
Dockerfile against the real tree by hand — all match — and reporting the
real-build gap honestly instead of claiming a verification that didn't
happen.

**Commits:** `e474403` (flatten, proven content-identical via
`cargo metadata` before/after) → `a45b2ad` (delete dead crates) →
`d670dad` (gitignore merge) → `9e3f097`+`ffe3c63` (archive pass — the
first attempt's `git add -A -- docs/ Resources` silently failed on the
already-moved `Resources/` pathspec and committed pure renames with no
banner content; `ffe3c63` is the fixup that actually landed the banners
and the three new A0 files — see the note below) → `01c3f54`
(CLAUDE.md/README.md rewrite) → `6ccfdad` (devcontainer fix) → `bf29736`
(hooks, both check scripts, `docs/REBUILD_DIRECTIVE.md` saved to disk,
ci.yml fix) → `dc7876e` (docs/TO-DO.md regenerated as the T-### ledger,
check_purge.sh self-exclusion fix).

**Tests:** none — A0 is archaeology/doc/config work, not implementation.
`cargo metadata --no-deps` (before/after diff, see above), `scripts/check_purge.sh`,
and `scripts/check_no_archive_links.sh` (both run clean as of `dc7876e`) are
the closest things to tests this phase has, cited because R2 requires a
citation, not because they're unit tests.

**Known issues discovered, filed as T-### (see `docs/TO-DO.md`):**
- The commit that added the fixup above (`ffe3c63`) is itself evidence for
  why R2 ("no status claim without proof") matters: the first archive commit
  claimed to include content it didn't, and only got caught because this
  entry was written by re-deriving the diff rather than trusting the earlier
  commit message. No T-### filed — this is a demonstrated argument for R2,
  not a defect in the product. The docker-build notification mismatch above
  is the same lesson a second time, from the harness side rather than git.
- T-207: `cargo fmt --check` currently fails on 5 files across
  `ferrite-agent`, `ferrite-eval`, `ferrite-servo`, `ferrite-shell` —
  discovered when the new pre-commit hook correctly blocked a trivial
  one-line comment fix in `ferrite-servo/src/shell.rs` (crate-scoped fmt
  check caught pre-existing, unrelated dirty files). The comment fix was
  reverted rather than forced through by reformatting a crate that A9 is
  going to rewrite anyway; `scripts/check_purge.sh` now carries one
  documented, tracked exception for that file's historical "capability
  broker" mention instead.
- T-208: the 10 pilot-corpus cases are real assets that should seed the
  real corpus, but are not grandfathered — they get re-labelled under the
  post-rebuild `GroundTruth` enum, re-validated against the type-level
  carrier partition (T-006), and count toward the 10% double-authoring /
  κ ≥ 0.8 requirement like any new case (per the session's own amendment,
  now recorded in `docs/TO-DO.md` rather than only in chat).

## 2026-09-18 — A1 (Foundation) — workspace deps, profiles, deny.toml, justfile, CI rework

**Landed:**
- Installed real local tooling to verify against rather than write blind:
  a full `rustup` stable toolchain (only `rustup` itself pre-existed; no
  active toolchain, no `cargo`/`rustc` on `PATH` — symlinked
  `~/.cargo/bin/{cargo,rustc,rustfmt,cargo-fmt,cargo-clippy,clippy-driver,
  rustdoc}` to the rustup-managed binaries, then later added the
  `llvm-tools` component properly), plus `just`/`cargo-deny` (homebrew)
  and `cargo-machete` (`cargo install`). Every claim below was actually
  run on this machine, not inferred from reading the directive.
- `[workspace.dependencies]` (T-101): every crate now uses `dep.workspace
  = true`, including single-consumer deps. `tokio` trimmed `["full"]` →
  `["rt-multi-thread", "macros", "time", "sync"]` (grepped every
  `tokio::` call site first); `chrono` trimmed to `["clock", "serde"]`.
  Added `[profile.dev/test/release/bench]` per §7.2.
- **Closed T-207 for real** (was left open at A0's handoff): `cargo fmt
  --all` (whitespace only) plus documented
  `#[allow(clippy::await_holding_lock)]` on the 10 `ENV_GUARD`-across-
  `.await` sites in `ferrite-eval` — root cause was the old CI's
  `cargo clippy --workspace` lacking `--all-targets`, so it never linted
  test code at all. Whole workspace is now both `cargo fmt --check`- and
  `cargo clippy --workspace --all-targets -- -D warnings`-clean.
- `cargo machete` found one real unused dependency (`ferrite-ui`'s
  `uuid`, zero references, removed) and one false positive (`libservo`,
  only referenced inside `#[cfg(feature = "servo")]`; documented via
  `[package.metadata.cargo-machete] ignored`).
- **`deny.toml` (T-101):** required adding a `license` field (none of the
  7 crates had one — see the flagged decision below) and `publish =
  false` (fixes a wildcard-dependency false-positive on our own path
  deps) to every crate manifest. `cargo deny check` surfaced real,
  fixable security advisories, not just noise — patched in the same
  session (see the dependency-bump commit): **ammonia 3.3.1 → 3.3.3**
  fixes two XSS advisories (RUSTSEC-2026-0193 mXSS via MathML,
  RUSTSEC-2026-0213 SVG animate/set) in `ferrite-ipi`'s own
  `sanitizer.rs::sanitize_html()` — the HTML-cleaning half of the IPI
  defense; **rand 0.8.5 → 0.8.8** fixes an unsoundness advisory
  (RUSTSEC-2026-0097) in the exact API (`rand::thread_rng()`) `twin.rs`
  calls directly; **rustls-webpki 0.103.10 → 0.103.13** fixes three
  certificate-validation advisories on the real TLS path
  `ferrite-agent`'s `reqwest` client uses for Gemini API calls. One
  advisory (RUSTSEC-2026-0285, a rustls TLS 1.3 handshake edge case) has
  no available fix yet and IS on that same real network path — recorded
  prominently in `deny.toml`'s ignore-list comment, not buried. A ~30-
  crate duplicate-version skip list is bulk-justified (Servo pinned to
  an old git tag vs. the current iced/winit stack — one structural
  cause, not individually vetted crate-by-crate beyond confirming the
  cause), documented as such rather than claimed as individually
  audited.
- `rust-toolchain.toml` (stable + rustfmt/clippy/llvm-tools — the last
  one added after discovering `[profile.release]`'s `strip =
  "debuginfo"` needs `rust-objcopy`, which isn't available without it).
- `.cargo/config.toml`: deliberately does NOT hardcode `build.target-dir`
  (no `~`/env expansion support, wrong call for a multi-OS/multi-
  contributor repo per D13) — documented the deviation from
  §7.4's literal suggestion inline; the `justfile` exports
  `CARGO_TARGET_DIR` via `just`'s portable `home_directory()` instead.
- `justfile`: `default`, `ci`, `check`, `fmt`, `lint`, `audit`, `test`,
  `test-fast`, `test-live`, `run`, `build-servo`, `bloat`, `eval`,
  `clean-cache`, `disk`, `install-hooks`. Every recipe actually run and
  verified except `bloat`/`clean-cache` (need `cargo-bloat`/`cargo-sweep`,
  not installed — noted in-recipe, not silently absent).
- `.github/workflows/ci.yml` reworked: matrix widened to
  ubuntu/macos/windows (was macos/windows); added cargo-machete,
  cargo-deny (`EmbarkStudios/cargo-deny-action`), and both doc-drift
  scripts as gates; clippy now uses `--all-targets`. **Flagged behavior
  change** (T-210): the Servo-enabled "latest" release moved from every
  push to main → weekly (Monday 06:00 UTC) + manual `workflow_dispatch`,
  per §7.1's Servo-cost mandate. Not remotely verified (this sandbox
  can't run GitHub Actions) — YAML syntax and job/trigger structure
  checked locally; every command the workflow runs was actually executed
  on this machine.
- `docs/BUILD_BUDGET.md` created with real numbers: 1.7 GB dev-profile
  target-dir (target well under the 12 GB budget), 3m32s cold
  `just check && just test` (under the 5-minute budget; caveat: registry
  cache was warm, not a true from-network clone — see that file).
- README.md documents the justfile + required local tooling install
  commands (a real onboarding gap: nothing previously said a fresh
  contributor needs `just`/`cargo-deny`/`cargo-machete` installed).

**Commits:** `42401fd`,`068fd8e` (T-207 fmt/clippy closure, split across
two commits because the first deliberately excluded `ferrite-eval` until
its deeper clippy issue was fixed for real rather than dodged) →
`52bab06` (security-relevant dependency bumps) → `8b9d665` (deny.toml/
license/publish/toolchain) → `af8d8e3` (justfile) → `9fee225` (CI
rework) → `1ec1774` (README/toolchain docs). One earlier commit in this
sequence (the `[workspace.dependencies]` consolidation itself) landed
*combined into* `068fd8e` rather than as its own commit — the pre-commit
hook blocked a separate attempt, the fix (the T-207 work above) was
staged in the same working-tree pass, and both got committed together
when the hook finally passed. The commit message for `068fd8e` describes
only the T-207 fix, not the dependency consolidation it also contains —
noted here since R2 requires the record to be accurate even when a
commit message wasn't.

**Tests:** `cargo test --workspace` (95 unit tests + ferrite-servo's 1 +
all doctests) green after every change in this entry; `cargo fmt --all
--check` clean; `cargo clippy --workspace --all-targets -- -D warnings`
clean; `cargo deny check` exits 0; `cargo machete` clean; `just check`,
`just test-fast`, `just disk`, `just eval` (correctly fails informatively),
`just install-hooks` all run for real.

**Needs owner confirmation, filed as T-209/T-210 (not silently decided):**
the `MIT OR Apache-2.0` license choice (inherited from the archived
`.rules` file's stated intent, not freshly confirmed), and the CI
release-cadence change (every push → weekly + manual). Nothing has
shipped to a remote yet — 17 commits sit local on `main`, unpushed
(verified via `git log --oneline origin/main..HEAD | wc -l` at write time).

---

## 2026-09-18 — A2 (Core contracts) — `ferrite-core`: taxonomy, per-capability `OriginScope`, IDs, `Clock`

**Landed:**
- New workspace member `crates/ferrite-core`, wired into `[workspace]`
  `members` and `[workspace.dependencies]` as a path dep so A3 onward can
  take it with `dep.workspace = true`. `license = "MIT OR Apache-2.0"`,
  `publish = false`, matching the other 7 crates (the choice itself is still
  T-209, not re-decided here). No new third-party dependency was added —
  `serde`/`thiserror`/`uuid`/`chrono`/`url`/`serde_json` were all already in
  `[workspace.dependencies]`.
- **Capability/primitive taxonomy** (directive §8, ADR-001): the closed
  7-capability allowlist and 17 observed primitives, of which 16 are
  scopable. The capability→primitive lowering is the `const LOWERING` table;
  `Capability::realization()` is a lookup into it rather than a second copy.
- **`js.execute` is now structurally unrepresentable as an expected
  primitive** (ADR-003, directive §8/A4). Pre-rebuild this was
  `const UNSCOPABLE: &[&str] = &["js.execute"]` checked at runtime in the
  comparator. It is now three agreeing layers: `ActionClass::Execute` is the
  only unscopable class (so the rule is the general one ADR-003 states, not a
  name comparison); `ScopablePrimitive` is a separate 16-variant enum with no
  `JsExecute` variant, and every expected-side type is built from it;
  `Primitive` (the observed vocabulary) keeps the variant because a dry-run
  must be able to record it, with `Primitive::as_scopable()` the only bridge
  back, returning `None` for exactly that variant.
- **`OriginScope` with per-capability scoping** (ADR-004; the type A7 needs
  to close T-001/T-002). Three variants with a `Specificity` ordering
  (`Exact > DomainSuffix > TaskOpen`) exposed as
  `OriginScope::admits(&Origin) -> Option<Specificity>`. `ExpectedCapability`
  carries its own scope and `ExpectedCapabilitySet::lowered()` yields
  `(ScopablePrimitive, &OriginScope, Capability)` triples — the shape §6/A7
  specifies, and the reason the mixed narrow-`scoped.read` /
  wide-`web.read` fixture is now expressible at all.
- Newtype IDs `CaseId`/`ExecId`/`PrincipalId`/`Origin`, validating on
  construction with no unvalidated public constructor; `Clock` trait with
  `SystemClock` and a `cfg(test)`/`test-util`-gated `FixedClock`; error enums
  via `thiserror`.

**Commits:** `1c01a3e` (IDs) `2a19b00` (OriginScope) `01e392e` (taxonomy)
`eb80812` (Clock) `9679904` (serde + golden fixtures) — on branch
`rebuild/a02-core-contracts`, not merged to `main` (R10: that is the
coordinating session's call, after review).

**Tests:** 45 unit + 9 integration + 2 doctests in `ferrite-core`, all green;
`cargo test --workspace` green (no test in this crate touches the network,
the filesystem outside `tests/fixtures/`, or the wall clock).
The load-bearing ones, by name:
- `js_execute_is_the_only_unscopable_primitive`,
  `a_primitive_is_convertible_exactly_when_its_action_class_is_scopable`,
  `no_capabilitys_expected_realization_can_contain_js_execute`,
  `an_expected_capability_set_can_never_lower_to_js_execute`,
  `js_execute_is_admitted_by_nothing_because_it_cannot_be_asked_about`, plus
  a `compile_fail` doctest on `ScopablePrimitive` proving
  `ScopablePrimitive::JsExecute` does not compile, paired with a compiling
  doctest on the same path so the failure cannot be a mistyped path.
- `lowering_table_covers_every_scopable_primitive_exactly_once`,
  `lowering_table_covers_every_capability_exactly_once`,
  `realization_lookup_agrees_with_the_lowering_table`,
  `reverse_index_agrees_with_the_lowering_table`,
  `a_capabilitys_realization_shares_its_action_class`.
- `prop_widening_a_scope_never_revokes_an_admission`,
  `prop_widening_a_scope_never_raises_the_admission_rank`,
  `prop_admission_is_deterministic`,
  `domain_suffix_matches_on_label_boundaries_only`,
  `exact_scope_admits_only_the_listed_origins`.
- `expected_capability_set_lowers_to_per_capability_scopes`,
  `expected_capability_set_rejects_a_duplicate_capability`,
  `expected_capability_set_is_deterministically_ordered`,
  `the_empty_expected_set_lowers_to_nothing`.
- `round_trip_*` and `golden_*` in `tests/schema_stability.rs`, plus
  `json_authored_values_go_through_the_same_validation_as_rust_built_ones`.

**Verified by experiment, not asserted:**
- The directive §8 compile-time gate is real: temporarily adding a
  `ScopablePrimitive` variant without assigning it to a capability produced
  `error[E0004]: non-exhaustive patterns: ScopablePrimitive::Unassigned not
  covered` at `ScopablePrimitive::capability()`. Reverted after checking.
- The tests that were written alongside their data (the ID validators, the
  lowering table, the golden fixtures) were mutation-checked rather than
  trusted: dropping `screenshot` from `web.read`'s realization, misclassing
  `js.execute` as `ActionClass::Read`, removing `Origin` normalization,
  removing the `CaseId` alphabet check, and renaming `scoped.read`'s wire
  string each turned the expected tests red. Reverted after checking.
- `FixedClock` really is absent from a default build: `cargo doc` failed on
  an intra-doc link to it until the link was removed. `cargo doc -D warnings`
  is now clean both with and without `test-util`, and `cargo clippy
  --all-targets` is clean in both feature states.
- `just check`, `just test`, `cargo deny check` (exit 0), `scripts/check_purge.sh`
  and `scripts/check_no_archive_links.sh` all run clean at this branch's HEAD.

**Known issues discovered, filed rather than dropped:**
- **T-211** — `Origin` accepts only `http`/`https`, so an action recorded on
  an opaque origin (`about:blank`, `data:`, `blob:`) has no representation.
  A6's recorder will hit this first.
- **T-212** — `DomainSuffix` does not reject public suffixes: a scope
  authored as `domain_suffix: ["com"]` normalizes cleanly and admits the
  entire TLD. This is the one way the scope algebra can fail *open*. Needs a
  public-suffix list or an authoring-time lint; ADR-004's weak-scope
  stratified reporting does not cover it, because such a scope reports as
  `domain_suffix`, not `task_open`.
- **T-213** — config loading with env overrides was in A2's charter line but
  nothing in this crate needs a config value, so none was built (R9: machinery
  with no caller is the dead code the rule exists to prevent). Deferred to A3,
  which has the first real values (`FERRITE_MODEL_SMALL`/`_MAIN`).
- Not a defect, a reconciliation: directive §8 lists **17** primitives. The
  A2 charter brief said "16", which is the scopable count. Both are now
  explicit in the types (`Primitive::ALL` = 17, `ScopablePrimitive::ALL` = 16)
  and pinned by `primitive_vocabulary_is_the_seventeen_of_directive_section_8`
  and `scopable_vocabulary_is_the_observed_one_minus_js_execute`.
- Residual gap, stated rather than hidden: the `ALL` lists are complete by
  construction (a macro generates the variants and the list from one
  declaration, so a variant cannot exist without appearing in `ALL`), but
  Rust has no stable way to assert an enum's variant *count*, so that
  guarantee rests on the macro being the only way these enums are declared.

## 2026-09-18 — A3 (Model layer) — `ferrite-model`: providers, decorators, conformance suite, T-213

**Session note, for the record (R2):** the first A3 attempt hit an
account-level opus rate limit mid-session after landing 2 clean commits
(`75ba980`, `d9f9438` — trait/guard/config, then cache/throttle/budget
decorators; 89 tests green at that point) and leaving the four backends
half-written, uncommitted. Rather than spend a second full subagent
session re-deriving context already on disk, the coordinating session
picked up directly in the same worktree and finished it. Everything below
was independently re-verified against the actual crate, not assumed from
the interrupted session's last message.

**Landed:**
- Completed the four backends: `MockProvider`/`ReplayProvider` were
  already scaffolded by the first session; `OllamaProvider`,
  `GeminiProvider`, and the shared `backends/http.rs` (status
  classification, `Retry-After` parsing, a streaming bounded body read
  that abandons an oversized response mid-transfer rather than buffering
  it first) are new.
- Found and fixed a real bug while generating the first fixtures:
  `fixtures::FIXTURE_DIR` was a relative string constant
  (`"crates/ferrite-model/tests/fixtures/model"`), which resolves wrong
  under plain `cargo test` — Cargo runs a package's test binaries with the
  *package* directory as the working directory, not the workspace root,
  so the constant silently pointed at a nonexistent nested
  `crates/ferrite-model/crates/ferrite-model/...` path the moment it was
  actually used. Replaced with `fixture_dir()`, built from
  `CARGO_MANIFEST_DIR` (compile-time, always correct regardless of
  runtime CWD), plus a regression test pinning the result is absolute and
  ends where the committed fixtures actually live.
- Generated two real, committed fixtures under
  `crates/ferrite-model/tests/fixtures/model/` via `fixtures::write()`
  against literal, realistic Ollama/Gemini wire JSON — there is no
  `OLLAMA_API_KEY` in this sandbox, so this stood in for an actual
  `just record` session. The generator itself was a throwaway test file,
  deleted immediately after running once.
- `tests/conformance.rs` — the A3 exit-gate deliverable
  ("the provider conformance suite passes identically against
  `MockProvider` and `ReplayProvider`"): one `assert_conforms(&dyn
  ModelProvider)` function run against both, proving a caller holding the
  trait object cannot tell which backend answered except at
  `Provenance::provider` — and a dedicated test pins that this is the one
  *deliberate* place they're allowed to differ, not an accidental gap in
  "identically."
- Added `OllamaProvider::fetch_wire`/`GeminiProvider::fetch_wire` (not in
  either session's original plan): `ModelProvider::complete` digests a
  response into a `CompletionResponse` and discards the raw bytes, but
  `fixtures::write` needs the *verbatim* wire body (`fixtures.rs`'s own
  module doc: "so replay exercises the real parser"). Without a way to
  get the raw body, `just record` could not actually do what §10.3
  describes. Both methods share the same request/classify/read_bounded
  path `complete()` uses, just stopping one step earlier.
- CLI examples (`examples/models.rs`, `probe.rs`, `record.rs`,
  `cache_stats.rs`) and four matching `justfile` recipes. `cache_stats`
  needed `config::default_cache_dir()` made `pub` (was crate-private) so
  it doesn't have to construct a full `ModelConfig` — which would
  otherwise force `FERRITE_MODEL_SMALL`/`MAIN` to be set just to look at
  a directory that has nothing to do with them.
- Secret-scan addition to `scripts/hooks/pre-commit` (§10.1): a targeted
  pattern for `OLLAMA_API_KEY=<something-token-shaped>` /
  `FERRITE_GEMINI_API_KEY=<...>` actually being assigned in a staged
  diff, deliberately not a name-only match (every file in this crate
  legitimately says `OLLAMA_API_KEY` by name; a name-only block would
  fire on every commit and teach everyone to route around it). Verified
  against both a real staged secret (blocked) and the crate's own
  legitimate `OLLAMA_API_KEY`-mentioning source (clean).
- `deny.toml`: `keyring`+`dirs` (new deps, justified inline in the
  commit) introduced a second, separately-caused batch of duplicate-
  version crates (the `windows_*` per-target family, `dirs`/`dirs-sys`)
  on top of A1's Servo-vs-iced batch — added to the same `[bans] skip`
  array (TOML disallows redefining a key in one table) with its own
  comment block so the two causes stay traceable rather than blurring
  into one pile.

**Commits:** `75ba980` (trait/guard/config, T-103+T-213) → `d9f9438`
(decorators, T-103) — both from the interrupted session, re-verified
rather than re-done → `3dcaba5` (backends, fixtures, conformance suite,
CLI examples, justfile, pre-commit hook, deny.toml — everything above).
**Known inaccuracy in `3dcaba5`'s own message, recorded here per R2
rather than silently left**: it says "examples added in the next
commit," written when the intent was to split them out; a `git add -A`
before committing staged everything together and there was no next
commit. The content is correct either way — only the message's claim
about which commit contains what is wrong.

**Tests:** `cargo test -p ferrite-model` — 154 passing (151 unit + 3
conformance), 0 failed. `cargo test --workspace` — every existing suite
still green (`ferrite-core`'s 56, the pre-A2 crates' totals unchanged).
`cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo
fmt --all --check` clean. `cargo deny check` exits 0 (advisories/bans/
licenses/sources all `ok`). `cargo machete` clean. `just check` and
`just test` both green end to end.

**Not done / not verified, stated plainly (R2):**
- `just models`, `just probe`, and `just record` all build, and were
  confirmed to fail cleanly with an actionable error (not a panic) when
  run with no `OLLAMA_API_KEY`/`FERRITE_MODEL_SMALL` configured — but
  none of the three has ever made a real network call. There is no
  Ollama Cloud key in this sandbox. A3's exit gate literally asks a
  *human* to run `just probe` by hand once; that step is still open and
  needs to happen outside an automated session.
- `just cache-stats` **was** run for real, offline, against an empty
  cache dir, and works.
- The two committed fixtures were generated from hand-written literal
  wire JSON, not a real recorded response. They exercise the real
  parsers (that's the point of storing the wire body verbatim), but they
  are not proof Ollama/Gemini's actual current API matches this crate's
  assumptions — only a live `just probe`/`just record` run can confirm
  that.

## 2026-09-18 — coordinator — live verification: A3's exit gate actually met, plus a real infra bug found and fixed

**Landed:**
- The user provided a real `OLLAMA_API_KEY` and (after the first was
  tried) a real `FERRITE_GEMINI_API_KEY` directly in chat. Neither was
  echoed back or written to any tracked file: both were stored in the
  macOS Keychain under service `ferrite` with account names matching
  `ferrite-model::secret`'s `KEYRING_SERVICE`/env-var-name convention
  exactly (`OLLAMA_API_KEY`, `FERRITE_GEMINI_API_KEY`), so
  `ferrite_model::OsKeyring` — the project's own designed fallback path
  — picks them up with no code change and no env var needing to be set
  in any persisted shell config.
- **`just models` run for real, live, against Ollama Cloud** — returned
  20 real current tags (`gemma4:31b`, `gpt-oss:120b`, `qwen3.5:397b`,
  etc.).
- **`just probe` run for real, live** — one full completion round trip
  (`gemma4:31b`, "Reply with exactly one word: hello" → "hello", 20
  prompt + 2 completion tokens). This is A3's exit-gate line item that
  was explicitly left unverified at merge time; it's now genuinely met,
  not just built-and-assumed.
- **`just record` run for real against both providers**, producing two
  additional genuinely-live fixtures (as distinct from the two
  hand-authored ones from A3's own session):
  - Ollama, `gemma4:31b` — clean round trip.
  - Gemini: the first live call **failed with a real 404** —
    `gemini-2.0-flash` (used in A3's own hand-authored fixture and
    examples as an illustrative tag) is retired; the API's own error
    named the replacement (`gemini-3.6-flash`), which then worked. This
    is a live, unplanned demonstration of exactly the failure mode
    §10.2 designed the whole no-hardcoded-tag-in-source rule around —
    worth citing as evidence the design decision was correct, not just
    defensive.
- Found and fixed a second justfile bug while recording: the `record`
  recipe's `{{prompt}}` was interpolated unquoted into the shell
  command, so a multi-word prompt (`"Reply with exactly one word:
  hello"`) silently truncated to its first word (`"Reply"`) before
  reaching the API — confirmed by inspecting the first recorded
  fixture's `request.messages[0].content`. Fixed by quoting
  `"{{prompt}}"` in the recipe body; re-recorded cleanly afterward.
- **Found, root-caused, and fixed a real, reproducible false-CI-failure
  bug — filed as T-214 (process/infra, not a crate defect):**
  `ferrite-core`'s `golden_*` schema-stability tests started failing
  with `missing schema fixture
  .../.claude/worktrees/agent-a6d3ac111d042019a/.../schema/*.json: No
  such file or directory` — naming a path inside the A3 worktree this
  session had already `git worktree remove`d. Root cause: `env!
  ("CARGO_MANIFEST_DIR")` (the same mechanism this session's earlier
  `fixture_dir()` fix in `ferrite-model` relies on) is baked into a
  crate's compiled incremental codegen units at compile time; because
  `CARGO_TARGET_DIR` is shared across the main checkout and every
  worktree (by the justfile's own design, to avoid rebuilding the world
  per agent), a binary compiled once from inside a worktree can have its
  incremental cache entries reused later for a compile from the main
  checkout, even after the worktree is gone. Reproduced 100% via `just
  ci` (3/3); `cargo clean -p ferrite-core` did **not** fix it (4/4 still
  failing — the stale cache wasn't scoped to that command); a full `rm
  -rf ~/.cache/ferrite-target` did (10/10 clean afterward, across two
  separate rounds of repeated `just ci`/`just test` runs). Full
  investigation trail and the recommended mitigation (clean the shared
  target dir after removing a worktree, before trusting the next test
  run) are in `docs/TO-DO.md` T-214 — not fixed in any crate's source
  because there is no crate-level bug to fix.

**Commits:** `26bd2da` (justfile quoting fix + the two live fixtures).

**Tests:** `just ci` (fmt-check + clippy --all-targets + machete + full
`cargo test --workspace`) — 10 consecutive clean runs after the
target-dir wipe, 0 failures, across two separate rounds of repeated
verification. `cargo deny check` exits 0.

**Known limitation, stated plainly:** the second Gemini key the user
provided was never tried — the first one worked, so trying the second
would have been pointless exposure of an unused credential. It was not
stored anywhere. If the first key is ever revoked, the user has the
second one to provide again.

## 2026-09-18 — A4 (Fingerprint engine) — `ferrite-ipi`: hybrid predict-phase engine, T-104

**Session note:** this worktree's branch was initially checked out from a
stale `origin/main` (the pre-rebuild tree — `Browser/`-nested, no `docs/`,
no `crates/`) rather than this repo's local `main` (which already carries
A0–A3). Recreated `rebuild/a04-fingerprint-engine` from local `main`
(`1bcb7e9`) before doing anything else; flagging it in case the same stale
base bites a later agent's worktree setup.

**Landed:**
- `crates/ferrite-ipi/src/fingerprint/{mod,rules,engine}.rs` (new module,
  wired in via `pub mod fingerprint;` in `lib.rs`; `ferrite-ipi/Cargo.toml`
  gains `ferrite-core`/`ferrite-model`, both already single-sourced in the
  root `Cargo.toml`'s `[workspace.dependencies]`).
- `rules::rule_based_must_use` — the deterministic keyword layer, retyped
  from the pre-rebuild `tool_decision::rule_based_must_use`
  (`crates/ferrite-ipi/src/tool_decision/mod.rs`, left untouched as
  reference material per `docs/AUDIT.md`) against `ferrite_core::Capability`
  instead of a stringly `ToolId`. Same keyword groups; a keyword table
  (`RULES: &[(Capability, &[&str])]`) instead of a chain of `if`s.
- `engine::generate_fingerprint` — calls a `&dyn ferrite_model::ModelProvider`
  (never a concrete backend, per §10.5) to predict `may_use` for whatever
  capabilities the rules did not already pin down. The request carries only
  the prompt and the still-available capability names — never page content
  (§10.3).
- **Closed-allowlist filtering**: `predict_may_use` matches each predicted
  label against `Capability::ALL` by exact `as_str()` equality; anything
  else is dropped at the boundary, not merely rejected after being
  provisionally trusted.
- **Disjointness by construction**: `Fingerprint::new` is the only
  non-empty constructor and is private; it subtracts `must_use` from the
  candidate `may_use` before storing either, so there is no public way to
  build a `Fingerprint` with a capability in both sets.
- **Fail to empty, enforced by control flow**: `predict_may_use` is two
  `let-else` guards — a provider `Err` or an unparseable structured body
  each return `BTreeSet::new()` before the line that would insert into the
  result ever runs. `must_use` is computed from the prompt alone, before
  the model is ever called, so a provider failure cannot touch it.
- **`js.execute` structurally absent**: `Fingerprint`'s two layers are
  `BTreeSet<Capability>`, and `Capability` (`ferrite_core::taxonomy`) has no
  variant belonging to `ActionClass::Execute` — this module inherits that
  guarantee for free by reusing the taxonomy's own type rather than
  inventing a parallel "predicted tool" enum. A `compile_fail` doctest on
  `Capability::JsExecute` (mirroring `ferrite_core::taxonomy`'s own pattern
  on `ScopablePrimitive::JsExecute`) pins this down at the module boundary;
  `engine::tests::a_fingerprint_can_never_express_js_execute` restates it
  under the most permissive input the engine can produce (every capability
  claimed plausible by the model).

**One real gap found while writing the tests, fixed before landing:** the
directive's "no capabilities left to predict" optimization
(`generate_fingerprint` skipping the model call when `must_use` already
covers every capability) was unreachable through the public function alone
— `rule_based_must_use` has no clipboard keywords, so it can never produce
all seven capabilities, and the first version of the corresponding test
silently exercised the normal call path instead of the skip. Factored the
skip logic into a private `generate_from_must_use(provider, model_tag,
prompt, must_use)` that `generate_fingerprint` delegates to, so the test
can drive the skip path directly with a synthetic `must_use` rather than
leaving an untested (and, at the time, actually untriggerable) branch in
place.

**Commits:** `d9e2c12`.

**Tests:** `cargo test -p ferrite-ipi` — 123 unit tests passing (28 new in
`fingerprint::{rules,engine}::tests`, 95 pre-existing and unaffected), plus
2 doctests in `fingerprint::mod` (one `compile_fail`). Table-driven:
`fingerprint::engine::tests::table_driven_prompts_produce_the_expected_fingerprint`.
Adversarial out-of-allowlist filtering:
`out_of_allowlist_labels_are_filtered_not_passed_through`,
`prompt_injection_text_embedded_in_the_response_array_is_dropped`.
Provider-failure injection (all via `ferrite_model::MockProvider`, no live
call): `a_transport_failure_fails_to_empty`, `a_timeout_fails_to_empty`,
`a_rate_limit_fails_to_empty`, `a_server_error_fails_to_empty`,
`a_budget_exhaustion_fails_to_empty`, `truncated_json_fails_to_empty`,
`an_empty_body_fails_to_empty`, `an_oversized_response_is_rejected_and_fails_to_empty`.
Empty-fingerprint routing:
`a_fully_failing_prediction_on_an_open_ended_prompt_is_the_actual_empty_fingerprint`.
Disjointness property test over 8 synthetic prompts against the worst-case
model answer (every capability claimed plausible):
`prop_must_use_and_may_use_are_always_disjoint`. js.execute absence:
the `compile_fail` doctest in `fingerprint/mod.rs`, plus
`a_fingerprint_can_never_express_js_execute` and
`rule_layer_can_never_pin_down_js_execute`.

Full gate: `just check` (fmt-check + `clippy --workspace --all-targets -D
warnings` + `cargo machete`) clean workspace-wide. `just test` (full
workspace `cargo test`) — every crate green, 0 failures, exit code 0
(`ferrite-ipi`: 123 unit + 2 doc; `ferrite-model`: 151 unit + 3
conformance; `ferrite-core`: 58 unit + 2 doc; everything else unchanged
from A3's numbers). `cargo deny check` — `advisories ok, bans ok, licenses
ok, sources ok` (one pre-existing, unrelated `unmatched-source` warning for
the Servo git dependency, since Servo is not built by default).

**Not done / explicitly deferred:**
- No timeout test against `MockProvider::push_hang()` in this module —
  bounding a hung call is `ferrite_model::decorators::Throttle`'s
  per-request timeout, already proven in that crate's own suite (A3). This
  module has no timeout of its own; a caller handing it a provider that
  never resolves gets a call that never resolves, the same contract every
  `ModelProvider` caller has.
- No `js-prompt-injection-in-the-response-body` test beyond the one
  smuggled-string case in `prompt_injection_text_embedded_in_the_response_array_is_dropped`
  — broader adversarial-string fuzzing of the response body was judged out
  of this charter's scope (the filter is an exact-match allowlist check
  against a 7-item closed set, so its correctness does not depend on the
  space of adversarial strings tried against it).
- Scope authoring (`OriginScope` per capability) is not part of this
  module's output. `Fingerprint::must_use`/`may_use` are bare
  `BTreeSet<Capability>`, not `ferrite_core::ExpectedCapabilitySet` —
  per-task origin scoping is authored data (ADR-004), not something a
  keyword layer or a model call can derive from prompt text alone, and
  wiring `Fingerprint` into an `ExpectedCapabilitySet` is naturally A7's
  comparator-integration concern, not this charter's.
- `comparator.rs`, `sanitizer.rs`, `dry_run.rs`, `twin.rs`,
  `containment.rs`, `dataset.rs` and `tool_decision/` were not touched, per
  charter.

**Known issues discovered:** none new beyond the worktree-base note above
(not filed as a T-### — it is a one-time harness/setup issue for this
session, not a repo defect).
