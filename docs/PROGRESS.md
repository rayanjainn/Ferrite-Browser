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

## 2026-09-18 — A5 — Sanitizer rebuilt as versioned data, live excision wired at one of two call sites

**Session note:** this worktree's branch was also initially checked out
from the same stale, pre-rebuild tree A4 hit (`Browser/`-nested, no
`docs/`/`crates/`, unrelated commit history — `git log --oneline -20`
showed things like "Task 17", "Git LFS", "CI bug fixing v1.0", nothing
resembling the rebuild). Recreated `rebuild/a05-sanitizer` from this
repo's local `main` (`97c7164`, which carries A0–A4) before doing anything
else, exactly as `docs/handoffs/a04.md` warned a future agent might need
to.

**Landed:**
- `crates/ferrite-ipi/src/sanitizer/{mod,patterns,detect,html,excise,config}.rs`
  (new module directory, replacing the old flat `sanitizer.rs` — Rust
  cannot have both `sanitizer.rs` and `sanitizer/mod.rs` resolve to the
  same `crate::sanitizer` path, and the charter's own framing ("rebuilding
  it as a new module") reads as a replacement, not an addition; the old
  file's logic and tests were folded into the new layout rather than
  archived-in-place per `docs/AUDIT.md`'s note that it is reference
  material to be reshaped).
- `patterns.rs` — `PatternDef`/`PatternSet` as versioned data (`id`,
  `description`, `regex`, `since_version`, a crate-wide
  `PATTERN_SET_VERSION`), replacing the old inline
  `general_injection_patterns()` tuple array. Two independent statics:
  `GENERAL_PATTERNS` (5 patterns, unchanged count from pre-rebuild, per
  the parameter rationale in `docs/REBUILD_DIRECTIVE.md` §13.4) and
  `SCRIPT_PATTERNS` (5 JS-specific exfiltration primitives).
- `detect.rs` — `Finding`/`LocatedFinding`, `detect()` against a
  `PatternSet`, `detect_injection_in_value()`'s recursive JSON walk with
  path tagging (`results[0].description`, not "somewhere in the blob").
  Golden corpus: two positive + two negative examples per pattern (10
  patterns total), table-driven, plus a test that every pattern has
  corpus coverage.
- `html.rs` — `ammonia`-based cleaning plus the comment/script carriers
  extracted from raw HTML before `clean()` runs, ported near-verbatim
  from the old `sanitize_html`.
- `excise.rs` — sentence-segment, tag-boundary-safe excision
  (`segment_bounds`/`merge_ranges`/`excise_ranges`, ported from the old
  code), single-space replacement (never a marker — argued in the module
  docs). New: an HTML-validity property test
  (`excision_never_produces_malformed_html`) over 12 hand-built HTML
  fixtures (injections mid-paragraph, tag-spanning, nested, at
  document start/end, inside table cells, next to self-closing tags,
  inside attribute-bearing elements), using `ammonia::clean()`'s own
  parse/serialize as the well-formedness oracle: the test asserts
  `clean(excise(clean(html))) == excise(clean(html))`, i.e. the excised
  output is already a fixpoint of ammonia's parser — reasoning for why
  that's a sufficient check is written into the test module's doc
  comment rather than asserted without justification.
- `config.rs` — `SanitizerConfig { detect_enabled, strip_enabled }` and
  `run()`, the single call that makes excision live: `run(&config, html)`
  returns a `SanitizedPage` whose `clean_html` is already excised when
  `strip_enabled` is set, instead of requiring a caller to remember a
  second `excise_*` call (the previous design's actual failure mode).
  `PROVISIONAL_FALSE_STRIP_RATE_CEILING = 0.02`, cited to `docs/TO-DO.md`
  T-203 as provisional, gates
  `benign_fixture_false_strip_rate_is_below_the_provisional_ceiling`,
  measured over a 35-sentence benign fixture corpus (ordinary web/tool
  prose, several deliberately close to trigger vocabulary) — the test
  passes at the current pattern set (0 of 35 stripped, well under the
  2% ceiling; the ceiling itself, not this run's exact rate, is what's
  provisional and needs A11's real audit against a real ~100-case benign
  corpus per §13.3).
- Independent-toggle tests: `detect_only_reports_findings_but_leaves_content_unchanged`,
  `strip_mode_reports_findings_and_returns_modified_text`,
  `off_mode_detects_nothing_and_returns_input_verbatim`.
- `crates/ferrite-ipi/src/tool_decision/mod.rs` — minimal, explicitly
  charter-sanctioned touch (the exception written into A5's charter for
  wiring `strip_enabled` live): added `DefenseMode::sanitizer_detect_enabled`/
  `sanitizer_strip_enabled`, and changed `ToolDecisionEngine::prepare_task`'s
  `On`/`SanitizerOnly` arms from calling the old detect-only
  `sanitizer::sanitize_html` to calling the new config-gated
  `sanitizer::run`. This closes D3/T-003 **at this one call site**: before
  the change, `On` and `SanitizerOnly` both only ever detected, never
  excised, at this call site — a second, previously-unnamed instance of
  D3's exact symptom, alongside the one the defect register names.

**Verified, not assumed:** `cargo test -p ferrite-ipi` — 142 unit tests
passing (up from the 123 A4's own PROGRESS.md entry above verified at its
landing — no other agent touched `ferrite-ipi` between A4 and this
session; net +19 after removing the old flat `sanitizer.rs`'s 26 tests and
adding more, restructured, in the new layout), 2 doctests unchanged (A4's
`fingerprint::mod` doctests). `cargo fmt -p ferrite-ipi --check` clean.
`cargo clippy -p ferrite-ipi --all-targets -- -D warnings` clean. `just
check` (fmt-check + `clippy --workspace --all-targets -D warnings` +
`cargo machete`) clean workspace-wide — this also proves `ferrite-eval`
and `ferrite-ui`, both of which depend on `ferrite_ipi::sanitizer::Finding`
and `ferrite_ipi::dry_run` (which itself calls into the new sanitizer
module), still compile and lint clean against the rebuilt API. `just
test` — every crate green, 0 failures, exit 0 (`ferrite-ipi`: 142 unit + 2
doc; every other crate's count unchanged from A4's numbers:
`ferrite-model` 151 + 3 conformance, `ferrite-core` 45 + 9 schema + 2 doc,
`ferrite-eval` 58 + 3 + 5, `ferrite-agent` 4, `ferrite-servo` 1). `cargo
deny check` — `advisories ok, bans ok, licenses ok, sources ok` (same
pre-existing, unrelated Servo-git-source `unmatched-source` warning A3/A4
also saw).

**A compatibility constraint discovered while porting, fixed before
landing:** `crate::dry_run::RecordingExecutor` (out of this charter's
scope) hardcodes the exact string `"instruction_override"` in its own
test `w1_t1a_comment_caught`. The initial pattern split gave the
"ignore ... previous/prior/above" rule and the "disregard ... previous"
rule two distinct new IDs (`instruction_override_ignore`/
`instruction_override_disregard`), which is the more correct design (the
two were previously silently sharing one label) but broke that test.
Since `dry_run.rs` cannot be touched per charter, the fix went the other
way: the "ignore" pattern kept the bare `instruction_override` id, and
only the "disregard" pattern got the new, previously-nonexistent
`instruction_override_disregard` id. Documented at the point of
definition in `patterns.rs`.

**Not done / explicitly deferred — read before assuming T-003 is fully
closed:** `docs/TO-DO.md` T-003 names a literal field, `strip_enabled`,
that lives in `crate::dry_run::RecordingExecutor`/`DryRunOrchestrator` —
a file this charter explicitly forbids touching. That flag still defaults
to `false`, and neither of its two current callers
(`ferrite-eval/src/harness.rs`'s `run_one`, which calls
`set_detect_enabled` but never `set_strip_enabled`; `ferrite-ui/src/lib.rs`,
which calls neither) ever flips it to `true` — both files are outside
`ferrite-ipi` and outside this charter's file list. **This means the
dry-run/eval-harness path — the one that actually produces
`ExecutionRecord`s and metrics — still runs with excision off**, even
though the sanitizer itself and its one in-crate production call site
(`tool_decision::prepare_task`) now support and use live excision
correctly. T-003 is marked `in-progress`, not `done`, in `docs/TO-DO.md`;
a new task (T-215) is filed for the remaining half, owned by A6
(`dry_run.rs`) and A12 (`harness.rs`, which already owns closing T-004,
itself dependent on T-003). Full detail in `crates/ferrite-ipi/src/sanitizer/mod.rs`'s
"How far this actually reaches" doc section and `docs/handoffs/a05.md`.

**Commits:** `58c9121`.

**Known issues discovered:** T-215 (new, see above) — `dry_run.rs`'s
`RecordingExecutor.strip_enabled` and its two production-shaped callers
never set it `true`; this is the literal field D3/T-003 names, and it
remains unwired outside this charter's reach.

## 2026-09-18 — A6 (Dry-run + twin + containment) — versioned modules, DEV_KEY removed, containment deleted

**Session note:** same hazard A4/A5 hit — worktree was initially checked
out on a stale, unrelated tree (`65b6d67`, "Task 17"/Git-LFS-era commits,
nothing to do with this rebuild). Recreated
`rebuild/a06-dry-run-twin-containment` from local `main` (`b229fb1`,
carries A0–A5) before doing any work.

**Landed:**
- `crates/ferrite-ipi/src/dry_run/{mod,record,content,executor,orchestrator}.rs`
  replaces the flat `dry_run.rs`, mirroring `fingerprint/`/`sanitizer/`'s
  layout. `RecordingExecutor`/`DryRunOrchestrator` are a faithful port of
  the old logic (ordered event log, per-origin `ReplyChannel` queues,
  inline detect/strip gating, whole-turn timeout returning a partial
  record) with three real additions: `DryRunRecord::events_with_seq()`
  (an explicit 0-based sequence number per event, derived from `Vec`
  position rather than a stored field — see below for why), a tested
  `ToolEvent::primitive()` bridge to `ferrite_core::Primitive`, and
  `DryRunOrchestrator::set_defense_mode(DefenseMode)`, which derives
  `detect_enabled`/`strip_enabled` together from `DefenseMode` the same
  way `sanitizer::run`'s config does (T-215, see below).
- `crates/ferrite-ipi/src/twin/{mod,key,crypto,manager}.rs` replaces the
  flat `twin.rs`. The compiled-in `DEV_KEY` constant is gone (D8).
  `key::resolve_twin_key` checks the OS keyring first, then
  `FERRITE_TWIN_KEY`, reusing `ferrite-model`'s `SecretStore`/`OsKeyring`/
  `Token`/`EnvSource` rather than reimplementing a keyring wrapper — see
  that module's doc comment for exactly what is and isn't reused, and why
  (`ferrite_model::secret::resolve()` itself returns `ModelError`, which
  would be a type lie for a twin key failure). `crypto::derive_key` turns
  resolved secret material into a 32-byte AES key via SHA-256 (no KDF —
  documented as sufficient because the payload is synthetic, not real).
  `TwinManager::load_or_generate` is now fallible
  (`Result<SyntheticTwin, TwinKeyError>`), directly tested for both the
  keyring-then-env order and the no-key-anywhere `Err` path (not a
  `#[should_panic]` standing in for it).
- `crates/ferrite-ipi/src/containment.rs` deleted outright (option (a),
  the directive's stated default for T-007/D7). `intercept_request()` was
  unreachable outside its own unit test — `RecordingExecutor` never made a
  real network call for it to intercept, so containment was already
  achieved by construction, not by an active interceptor. Confirmed
  `RecordingExecutor`'s own source has no network-capable dependency
  (`this_modules_source_names_no_network_capable_dependency`, grep-backed
  against split literals so the test can't trivially match its own
  prose). Dropped the now-dead `nix` dependency
  (`[target.'cfg(target_os = "linux")'.dependencies]`) from
  `ferrite-ipi/Cargo.toml`, verified unused workspace-wide with `cargo
  machete`, and removed the matching `cargo-deny` skip entry for `nix`
  (it was the only workspace consumer).

**Deviation from the directive's illustrative `ToolEvent { primitive,
origin, seq }` shape**, and why: `crate::comparator` (A7's charter,
explicitly off-limits this session) already pattern-matches
`event.tool: ToolId` in `compare()` and constructs events via
`record_tool(ToolId::new(..), ..)` in its own tests; `crate::dataset`
(A11's charter, also off-limits) constructs a bare `ToolEvent { tool,
origin }` struct literal directly in a `#[cfg(test)]` fixture. Renaming
`tool` to `primitive: ferrite_core::Primitive` or adding a mandatory
`seq: u64` field would not compile without editing both files. Kept
`ToolEvent { tool: ToolId, origin: Option<String> }` unchanged; added
`ToolEvent::primitive()` (tested bridge to `ferrite_core::Primitive`,
`None` for exactly one known mismatch — `BrowserTool::DownloadFile`'s
`ToolId` wire string is `"download.file"`, `Primitive::Download`'s is
`"download"`) and `DryRunRecord::events_with_seq()` (derived seq via
`Vec` position) so A7 gets the forward-compatible shape without a
compile break today.

**Deviation from a literal "fail loudly, `Result::Err`" reading for the
twin key**, found empirically, not assumed: making
`DryRunOrchestrator::run()` hard-propagate a missing-key error broke five
`ferrite-eval` tests (`harness::e2e_tests::*`, `corpus::loaded_case_runs_through_the_full_pipeline`)
that call `run_one`/`run_case` with no `FERRITE_TWIN_KEY` configured
anywhere in the test environment — exactly the R7 "no operator secret
required for the test suite" expectation the codebase holds for model
provider keys, which nothing established for the twin key before this
session. `TwinManager::load_or_generate` still returns the strict,
directly-tested `Result::Err` (T-008's literal requirement, satisfied at
that layer). `DryRunOrchestrator::run` — the layer `ferrite-eval`/
`ferrite-ui` actually call, neither editable this session — instead logs
a warning and generates an unpersisted twin for that call, mirroring
`tool_decision`'s existing "no Gemini key configured" fallback pattern
elsewhere in this crate. Reasoning recorded in
`DryRunOrchestrator::run`'s doc comment and `docs/handoffs/a06.md`: the
twin key gates a caching convenience for synthetic data, not any part of
the containment/sanitizer/consent mechanism, so losing the cache is the
correct failure mode, not losing the whole dry run.

**T-215, this session's half:** `DryRunOrchestrator::set_defense_mode`
derives `detect_enabled`/`strip_enabled` together from `DefenseMode` in
one call, reusing `tool_decision::DefenseMode::sanitizer_detect_enabled`/
`sanitizer_strip_enabled` (A5). A12's remaining one-line fix in
`ferrite-eval/src/harness.rs::run_one` (line ~220): replace
`orch.set_detect_enabled(behavior.detect_enabled);` with
`orch.set_defense_mode(mode);` — `mode: DefenseMode` is already a
parameter of `run_one`, and nothing later in that function reads
`behavior.detect_enabled` again, only `behavior.loop_runs`. `docs/TO-DO.md`
T-215 updated accordingly; still not fully closed until A12 makes that
change (this session cannot touch `harness.rs`).

**Verified:** `cargo test -p ferrite-ipi` — 140 unit + 2 doctests, 0
failures. `cargo fmt -p ferrite-ipi --check` and `cargo clippy -p
ferrite-ipi --all-targets -- -D warnings` clean. `cargo machete` clean for
`ferrite-ipi`. `cargo build --workspace` and `cargo test --workspace`
green (0 failed) after the `Box<dyn EnvSource + Send + Sync>` fix below.
`just check` (fmt-check + clippy --all-targets -D warnings + cargo
machete) and `just test` (full workspace suite) both green from the repo
root. `cargo deny check`: `advisories ok, bans ok, licenses ok, sources
ok`, only the pre-existing Servo-git-source warning remains (same one
A3/A4/A5 logged).

**Bug found and fixed mid-session, before the workspace-wide run above
ever went green:** `TwinManager`'s injectable `Box<dyn EnvSource>` field
is not `Send`/`Sync` by default (`EnvSource`, in `ferrite-model`, carries
no such supertrait, unlike `SecretStore`, which already does). `cargo
build --workspace` failed on `ferrite-ui`, which holds a
`DryRunOrchestrator` across an `.await` inside a `tokio::spawn`'d agent
loop task. Fixed by bounding the trait object explicitly at every
declaration site in this crate (`Box<dyn EnvSource + Send + Sync>`) —
every concrete source (`SystemEnv`, `MapEnv`) already satisfies it, so
this costs nothing real. Not filed as a new T-### since it was fully
fixed within this session and is now covered by the green workspace-wide
build.

**Commits:** `be891b8`, `f8edef3`.

**Tests:** `dry_run::orchestrator::tests::dry_run_records_navigation_and_updates_origin`,
`dry_run::orchestrator::tests::origin_updates_across_a_scripted_navigation`,
`dry_run::orchestrator::tests::same_origin_sequential_reads_get_different_scripted_content`,
`dry_run::orchestrator::tests::detect_and_strip_gating_actually_gates_content`,
`dry_run::orchestrator::tests::timeout_returns_a_partial_record_not_an_empty_one`,
`dry_run::orchestrator::tests::set_defense_mode_derives_both_flags_from_on`,
`dry_run::orchestrator::tests::missing_twin_key_degrades_the_dry_run_rather_than_failing_it`,
`dry_run::record::tests::seq_is_monotonic_and_matches_call_order`,
`dry_run::executor::containment_tests::this_modules_source_names_no_network_capable_dependency`,
`twin::key::tests::the_keyring_is_consulted_first`,
`twin::key::tests::neither_source_is_a_typed_error_naming_both_and_never_a_panic`,
`twin::manager::tests::manager_without_a_resolvable_key_fails_loudly_not_silently`,
`twin::crypto::tests::encrypt_decrypt_roundtrip_with_a_resolved_key`.

**Known issues discovered:**
- T-215 updated, not closed — see above; A12 owns the remaining one-line
  `harness.rs` change.
- New: `ToolEvent::primitive()` returns `None` for
  `BrowserTool::DownloadFile` because `BrowserTool::tool_id()`
  (`ferrite-agent`) emits `"download.file"` while
  `ferrite_core::Primitive::Download`'s wire string is `"download"` — a
  pre-existing mismatch between the two crates' vocabularies, predating
  this session, out of scope for both (neither `ferrite-agent` nor
  `ferrite-core` is in this charter's file list). Filed as T-216 in
  `docs/TO-DO.md`, owned by A7 (needs it to consume `ToolEvent::primitive()`
  cleanly) or whoever next touches `BrowserTool::tool_id()`.
- New: T-211 (opaque origins) is exercised but not closed by this
  session. `dry_run::record::extract_origin` records an opaque-scheme URL
  (`about:blank`, `data:`, ...) as its literal string rather than a
  normalized origin, since `ferrite_core::Origin::parse` structurally
  rejects every scheme but `http`/`https`. Documented in
  `dry_run::record`'s module docs as the deliberate interim behavior: a
  literal opaque string will also fail `Origin::parse` downstream, which
  is the *correct* effect (unadmittable by any scope, like `js.execute`)
  achieved by type mismatch rather than a principled `Origin::Opaque`
  variant. `ferrite-core` is out of this session's scope; T-211's row in
  `docs/TO-DO.md` is updated with this note but stays `open`, owned by
  A7/whoever next touches `ferrite-core::Origin`.

## 2026-09-18 — A7 (Comparator + origin model) — per-capability `compare()`, admission-rank attribution, T-001/T-002/T-206/T-211/T-216

**Session note:** same hazard A4/A5/A6 hit — worktree started on a stale,
unrelated tree (`65b6d67`, "Initial test case designing..."/Git-LFS
history unrelated to Ferrite). Recreated
`rebuild/a07-comparator-origin-model` from local `main` (`69f4268`,
carries A0–A6) before doing any work.

**Landed:** `crates/ferrite-ipi/src/comparator/{mod,expected,diff,consent,
legacy}.rs` replace the flat `comparator.rs`. `compare()`'s new contract:
`fn compare(expected: &ExpectedFingerprint, actual: &DryRunRecord) ->
FingerprintDiff`. `ExpectedFingerprint` wraps
`ferrite_core::ExpectedCapabilitySet` (A2) — capabilities, each carrying
its own `OriginScope` — and lowers through
`ExpectedCapabilitySet::lowered()` directly rather than re-deriving the
capability→primitive table. The old file's local `OriginScope`/
`ScopeType` duplicate types are gone entirely; every scope-typed value in
this module is `ferrite_core::scope::OriginScope`/`Specificity`.

**T-001/D1 (closed, `6cb4c70`):** a fingerprint mixing a narrow
`scoped.read` on `mail.example.com` with a wide `web.read` on
`task_open`/a domain suffix is now expressible and attributes correctly
per event — proven directly in
`comparator::tests::narrow_scoped_read_and_wide_web_read_in_the_same_fingerprint_attribute_independently`
and the exhaustive 2×3×4 enumeration
(`comparator::tests::exhaustive_two_capability_three_origin_four_primitive_enumeration`).

**T-002/D2 (closed, `6cb4c70`):** `OriginScope::admits`'s `Specificity` is
consumed, not just computed — `compare` scans every admitting candidate
for a matching primitive and keeps the one with the *maximum* rank
(`exact > domain_suffix > task_open`), recording it on the `Attribution`.
`comparator::tests::admission_rank_is_actually_consumed_not_just_computed`
and `comparator::tests::admission_rank_picks_the_tighter_of_two_admitting_capabilities`
are written to fail if `compare` reverted to "first match wins" instead
of "highest-`Specificity`-wins".

**T-206 (closed, `6cb4c70`):** the double-flag bug (an event landing in
both `extra_primitives` and `out_of_scope_origins`) is fixed by
construction: "primitive named by some capability" and "primitive named
by no capability" partition the whole primitive space independently of
origin, so `compare`'s classification is an `if`/`else`, not two
independent checks.
`comparator::tests::t206_regression_primitive_not_expected_and_origin_not_admitted_lands_in_exactly_one_bucket`
is the direct regression test. No `Both` variant was added — see
`comparator`'s module docs ("Why there is no `Both` variant") for why
that would either duplicate `ExtraPrimitive` or resurrect the bug under a
new name.

**T-211 (ratified, `6cb4c70` — not "resolved by adding a variant"):**
after reading `ferrite_core::ids::Origin`'s doc comment together with
ADR-004 and A6's handoff, this session concludes the existing behavior
(`Origin::parse` accepts only `http`/`https`, so an opaque-scheme URL
fails to parse, which is the correct "unadmittable by any scope" signal)
*is* the intended design, not a gap — adding an `Origin::Opaque` variant
would need to be unconditionally unadmittable to mean anything, which
"fails to parse" already achieves with less surface area. `compare`
treats a recorded-but-unparseable origin string as `out_of_scope_origins`
(there is a string to report) and a wholly-missing origin (`None`) as
`extra_primitives` (nothing to report). Tests:
`comparator::tests::an_opaque_scheme_origin_is_out_of_scope_under_its_literal_string`,
`comparator::tests::an_event_with_no_recorded_origin_is_an_extra_primitive`.
`docs/TO-DO.md`'s T-211 row is updated to `done` (ratified), not left
open as an unresolved question.

**T-216 (closed, `6cb4c70`):** `download.file` (`ferrite-agent`'s
`BrowserTool::DownloadFile::tool_id()`) vs. `Primitive::Download`'s wire
string `"download"` is resolved with a one-entry mapping local to
`comparator::resolve_primitive`, not by changing `BrowserTool::tool_id()`
— that string is key material for `dry_run::content::DryRunContent::
by_tool_id`'s per-tool reply queues (A6's file, off-limits) and is
embedded literally in `ferrite-eval`'s corpus fixtures/tests, so changing
it would ripple far outside this charter for no benefit over a targeted
mapping. Test:
`comparator::tests::download_file_attributes_to_web_download_at_an_admitted_origin`.

**The design gap (A4's `Fingerprint`/legacy `ToolFingerprint` carry no
per-capability scope) — resolved, not papered over:**
`ExpectedFingerprint::from_fingerprint`/`from_legacy_tool_fingerprint`
apply one scope to every capability in a fingerprint that has none of its
own, sourced from the task's context URL: `exact([context_url])` when
present, `task_open` with a stated provisional rationale when absent —
**never an unconditional `task_open`**, which would silently readmit
everything and make this whole rewrite pointless.
`comparator::expected::tests::from_fingerprint_with_a_context_url_still_flags_a_different_origin`
is the regression test that would fail if this default were ever widened
to ignore `context_url`. `ExpectedFingerprint::from_capabilities` is the
direct, no-defaulting constructor for a caller with a genuine
per-capability scope of its own — reasoning at length in
`comparator`'s module docs ("Where a per-capability `OriginScope` comes
from"). As a direct consequence, the live `ferrite-ui` agent-task path's
`compare()` call site — previously a hardcoded, unconditional
`OriginScope::task_open()` behind a `TODO(Task 19)` comment — now derives
a real exact-scope from `agent_task.context_url` when one exists
(`ferrite-ui/src/lib.rs`, commit `40221de`).

**Cross-crate compile fixes (minimal, `40221de`), all required by the new
`compare()` signature and the deletion of the old local `OriginScope`:**
- `ferrite-eval/src/harness.rs::run_one` — context_url now extracted from
  `OriginScope::Exact`'s variant instead of a removed `.exact` field;
  builds an `ExpectedFingerprint::from_legacy_tool_fingerprint` before
  calling `compare`.
- `ferrite-eval/src/adjudication.rs`, `harness.rs` test fixtures —
  migrated off the deleted `comparator::OriginScope` to
  `ferrite_core::scope::OriginScope`'s fallible `exact`/`task_open` API
  (mechanical, test-only call sites).
- `ferrite-eval/src/corpus.rs`, `tests/pilot_w6.rs`,
  `tests/pilot_corpus/*.json` (10 files) — `expected_origins` JSON
  literals updated to `ferrite_core::OriginScope`'s externally-tagged
  wire shape (`{"exact": [...]}` / `{"domain_suffix": [...]}` /
  `{"task_open": {"rationale": ...}}`) in place of the old four-key flat
  struct; these are authored test fixtures, not corpus/ or dataset.rs
  schema, so within this session's scope to fix mechanically.
- `ferrite-eval` and `ferrite-ui` both gained a direct `ferrite-core`
  dependency (confirmed present in the workspace root `Cargo.toml`
  first) — a downward edge only, `cargo machete` confirms no unused
  dependency resulted.
- `dataset.rs`'s `CaseDefinition::expected_origins`/
  `ExpectedRealization::origin_scope` fields are now typed
  `ferrite_core::scope::OriginScope` (three call sites plus an import;
  the struct shapes themselves are untouched — no feature work in A11's
  schema beyond what compiling required).

**`FingerprintDiff`'s shape:** `extra_primitives: HashSet<ToolId>` and
`out_of_scope_origins: HashSet<String>` keep their pre-rebuild names/types
verbatim (read directly by `dataset::unscopable_primitive_invoked` and
extensively by `ferrite-eval`/`ferrite-ui`, none of which this charter's
"minimal mechanical" mandate permitted renaming). New:
`attributions: Vec<Attribution>` (`#[serde(default)]`, so pre-A7 JSON
still deserializes), each entry naming the tool, origin, admitting
`Capability`, and winning `Specificity` — the shape A8's audit log needs
to cite "which capability justified this action" per entry.

**Tests:** 37 new tests in `crates/ferrite-ipi/src/comparator/` (mod.rs:
20, expected.rs: 6, diff.rs: 6, consent.rs: 3, legacy.rs: 2), including
property tests for monotonicity
(`comparator::tests::prop_widening_a_capabilitys_scope_never_turns_an_admitted_event_into_a_flagged_one`)
and determinism (`comparator::tests::prop_compare_is_deterministic`), the
js.execute-no-exceptions test
(`comparator::tests::js_execute_is_flagged_even_under_a_scope_that_would_admit_any_origin`),
and the exhaustive small-case enumeration named above.

**Verified:** `cargo test -p ferrite-ipi` — 166 unit tests + 2 doctests
(1 ignored — the module-doc `compare()` signature snippet is
`` ```ignore ``` ``), 0 failures. `cargo fmt -p ferrite-ipi --check` and
`cargo clippy -p ferrite-ipi --all-targets -- -D warnings` clean. `cargo
build --workspace` and `cargo test --workspace` green (0 failed, verified
via `grep -E "FAILED|error\["` over the full run — no matches). `cargo
fmt --all --check` and `cargo clippy --workspace --all-targets -- -D
warnings` clean. `just check` (fmt-check + clippy + `cargo machete`)
green — machete confirms no unused dependency from the two new
`ferrite-core` deps. `just test` (full workspace suite) green. `cargo
deny check`: `advisories ok, bans ok, licenses ok, sources ok`, only the
pre-existing Servo-git-source warning (same one A3–A6 logged).

**Commits:** `6cb4c70` (comparator rebuild), `40221de` (cross-crate
compile fixes).

**Known issues discovered / left for later agents:**
- `ExpectedFingerprint::from_fingerprint`/`from_legacy_tool_fingerprint`'s
  scope-sourcing policy is explicitly provisional: it applies one scope
  (derived from a single context URL) uniformly to every capability in a
  fingerprint, because neither `fingerprint::Fingerprint` nor the legacy
  `tool_decision::ToolFingerprint` distinguishes which capability wants
  which origin. A real per-task authoring mechanism (e.g. wiring
  `dataset::CaseDefinition`'s `scope_rationale` into a genuine
  per-capability scope, or a UI for authoring one) would let
  `ExpectedFingerprint::from_capabilities` express real
  precision — filed as new `T-217` in `docs/TO-DO.md`.
- `ferrite-ui`'s `run_agent_loop`/consent path (`ConsentDecision`,
  `FingerprintDiff::summary()`) is unchanged by this session beyond the
  one call site fixed to compile — A10's UI charter is the next place
  that surface gets real attention (e.g. showing `Attribution`'s
  capability/specificity in the consent panel, not just the flagged
  sets).
- A8 (audit log) is the next consumer of this module's output: see
  `docs/handoffs/a07.md` for exactly what shape `FingerprintDiff`/
  `Attribution` now hand it.

## 2026-09-18 — A8 (Audit log) — full-payload hash preimage, tamper matrix, T-005/T-108

**Session note:** same hazard A4–A7 hit — worktree started on a stale,
unrelated tree (`65b6d67`, "Initial test case designing..."/unrelated
history, different project entirely). Recreated `rebuild/a08-audit-log`
from local `main` (`dd5fd71`, carries A0–A7) before doing any work.

**Landed:** `crates/ferrite-audit-log/src/lib.rs` — `compute_entry_hash`
is now the single function `AuditLog::append` and `AuditLog::verify_chain`
both call, so the two can never drift apart the way the old duplicated
`format!("{}{}{:?}{}{}", ...)` strings implicitly could. The preimage now
covers every persisted field, not just five of nine: `schema_version`
(new — `AUDIT_SCHEMA_VERSION = 1`, itself hashed), `entry_id`, `sequence`,
`timestamp` (as its RFC3339 string, not `chrono`'s internal repr),
`kind`, `principal_id`, `capability`, `url`, `prev_hash`. Serialization is
`serde_json::to_vec` over a purpose-built `HashPreimage` struct with fixed
field order (not `Debug` formatting, not a `HashMap`) — this closes the
two real ambiguity risks named in the charter: `{:?}` on an enum is not a
stable wire format across derive versions, and JSON distinguishes `null`
from `""` and length-delimits every string, so `None` vs.
`Some(String::new())` for `capability`/`url` can never collide the way
naive concatenation could. The preimage is domain-separated with a fixed
`b"ferrite-audit-v1"` prefix (`HASH_DOMAIN`).

`PersistentAuditLog` gained a second, small persisted checkpoint —
`audit_chain_head`, a single-row table holding the next sequence number
and last entry hash, written in the same SQL transaction as every
`append` — because the hash chain alone cannot prove *completeness*
(nothing in entry N's hash can prove entry N+1 once existed and was
deleted). `load` cross-checks this watermark against the entries actually
present and returns the new `AuditError::TruncatedChain` variant on
mismatch. This is a best-effort defense, not a cryptographic guarantee —
documented plainly in the module docs and in a test
(`truncation_undetected_if_attacker_also_forges_the_checkpoint_head`)
that shows exactly where it stops working; filed as **T-218** for whoever
owns the deployment threat model to decide whether an external anchor is
worth building.

**Commits:** `6250cb3` (fix(audit-log): cover full canonical payload in
the hash preimage).

**Tests:** 21 tests in `crates/ferrite-audit-log/src/lib.rs` (0 before
this session — the crate had no test module at all):
- Tamper matrix, one test per persisted field, each asserting
  `verify_chain()` returns `false` (T-005's direct regression, covering
  every field, not just the two the bug report named):
  `tamper_matrix_capability`, `tamper_matrix_url`,
  `tamper_matrix_principal_id`, `tamper_matrix_kind`,
  `tamper_matrix_timestamp`, `tamper_matrix_sequence`,
  `tamper_matrix_entry_id`, `tamper_matrix_prev_hash`,
  `tamper_matrix_schema_version`, `tamper_matrix_entry_hash_rewritten_directly`
  (10 tests; two more, `tampering_capability_after_the_fact_breaks_verification`
  and `tampering_url_after_the_fact_breaks_verification`, are the literal
  pre-fix bug scenario, kept as an explicit named regression).
- Truncation: `truncation_attack_dropping_the_tail_is_detected_on_load`
  (positive case) and `truncation_undetected_if_attacker_also_forges_the_checkpoint_head`
  (the documented limit, asserted as `Ok`, not a false claim of full
  coverage).
- Reordering: `reordering_attack_swapping_sequence_values_is_detected_on_load`
  (swaps two entries' `sequence` column values directly in SQLite,
  leaving stored hashes untouched — caught because `sequence` is now
  hashed).
- Insertion: `insertion_attack_splicing_a_self_consistent_forged_entry_is_detected_on_load`
  (a forged row whose own hash is internally valid, computed with the
  real `compute_entry_hash`, chained from a real predecessor but never
  woven into the successor's `prev_hash` — the chain-link check catches
  it regardless of where SQLite's tie-broken ordering places it).
- Golden fixture: `golden_chain_fixture_pins_entry_hashes` — two
  fixed-input entries (fixed UUIDs, fixed RFC3339 timestamp, fixed
  capability/url) with pinned expected hex:
  `c1b4ff371b8ed37e43a3f6fb61a4a88f95c457cfd3219e0214d743929b0de11a`
  (entry A, `EvalExecutionRecorded`) and
  `66e21de3ecb7547adc3bcf6a901031fe694b3a3cd73c37e26b4d263e4c282671`
  (entry B, `CapabilityExercised`, chained from A). An accidental
  preimage change now fails loudly instead of silently changing what old
  audit logs mean.
- Persistence sanity: `persisted_log_round_trips_and_verifies`,
  `persisted_tamper_of_capability_column_is_detected_on_load` (the exact
  T-005 SQLite-column-rewrite scenario, now caught on `load`),
  `append_produces_a_verifying_chain`, `empty_log_verifies`.

**Verified:** `cargo test -p ferrite-audit-log` — 21 passed, 0 failed, 0
doctests. `cargo fmt -p ferrite-audit-log --check` and
`cargo clippy -p ferrite-audit-log --all-targets -- -D warnings` clean.
`cargo build --workspace` and `cargo test --workspace` green (checked via
`grep -E "FAILED|error\[" ` over the full output — no real matches; the
only "error" substring hits are test names like
`only_a_rate_limit_carries_a_retry_after` from `ferrite-model`). `just
check` (fmt-check + clippy --all-targets -D warnings + cargo machete)
green — machete reports no unused dependency. `just test` (full workspace
suite) green. `cargo deny check`: `advisories ok, bans ok, licenses ok,
sources ok`, only the pre-existing Servo-git-source `unmatched-source`
warning already logged by A3–A7.

**Cross-crate compile touches:** none. `PersistentAuditLog::new`/
`append`/`load` and `AuditLog::append`/`verify_chain`'s signatures are
unchanged; `AuditEntry` gained a `pub schema_version: u8` field, but no
crate outside `ferrite-audit-log` constructs `AuditEntry` by struct
literal (confirmed via `grep -rn "AuditEntry\s*{" crates/` — only the two
in-crate construction sites) or destructures it, so this is source- and
binary-compatible for every caller (`ferrite-servo`, `ferrite-shell`,
`ferrite-ui`, `ferrite-eval` — confirmed via
`grep -rln "ferrite_audit_log" crates/`, all of which only call
`append`/`verify_chain`/`new`/`load` and read `AuditEntry` fields, never
construct one).

**On-disk audit databases:** none exist anywhere in this repo —
confirmed via `git ls-files | grep -iE '\.(db|sqlite|sqlite3)$'` (no
matches) and a filesystem search excluding `target/`/`.git/` (no
matches). So the fact that this session's preimage change makes any
*hypothetical* pre-A8 on-disk audit log fail `verify_chain()`/`load` is a
real, correct, and now-legible consequence of `schema_version` existing
(a future verifier can branch on it) — but it invalidates nothing that
actually exists today.

**Known issues discovered / left for later agents:**
- **T-218** (filed above, in `docs/TO-DO.md`): the new
  `audit_chain_head` checkpoint is a best-effort watermark, not a
  cryptographic completeness guarantee — an attacker with full SQLite
  read/write access can forge it consistently with a truncated tail.
  Closing this needs an external anchor; out of this charter's scope.
- **T-219** (filed above): `AuditError::HashMismatch`/`ChainBroken` were
  already declared-but-unconstructed before this session (`verify_chain`
  returns a bare `bool`, never those variants) and remain so — not
  `dead_code`-lint-visible because they're variants of a `pub` enum, but
  genuinely unreachable from any current call path. Fixing this properly
  means changing `verify_chain`'s return type to
  `Result<(), AuditError>`, a public API change touching 3 external call
  sites currently using it as a boolean — real, small, but outside this
  charter's "fix the preimage" scope.
- No existing on-disk audit databases anywhere in this repo (see above) —
  confirmed, not assumed.

**Exact next action for A9:** `crates/ferrite-engine`,
`crates/ferrite-engine-servo`, `crates/ferrite-agent`, per §6/A9
(T-109). **Checked this session:** `crates/ferrite-engine` and
`crates/ferrite-engine-servo` do **not** exist yet — absent from root
`Cargo.toml`'s `members` list and from disk; A9 creates both from
scratch. `crates/ferrite-agent` already exists as a workspace member
(`src/lib.rs`, `src/gemini.rs`). Separately, `crates/ferrite-servo`
already exists too, but it is a *different* thing — the winit/Iced
shell's own Servo integration behind its `servo` Cargo feature, not the
directive's `BrowserEngine`-trait `ferrite-engine-servo` — A9 should read
both before naming or wiring anything, the same kind of crate-identity
check this session's own charter needed (`ferrite-audit` vs. the real
`ferrite-audit-log`).

## 2026-09-18 — A9 (Engine + agent action surface) — `ferrite-engine`, `ferrite-engine-servo`, `ferrite-agent::browser_loop`, real `just build-servo`, T-109

**Commits:** `aaae747` (feat(engine): BrowserEngine trait + MockEngine),
`8a1b7e3` (feat(engine-servo): ServoEngine wrapping HeadlessServoSession),
`3df9af2` (feat(agent): engine/provider-agnostic browser_loop).

**Session note:** same hazard A4–A8 hit — worktree started on a stale,
unrelated tree (`65b6d67`, "Initial test case designing..."). Recreated
`rebuild/a09-engine-agent` from local `main` (`b4f4c8c`, carries A0–A8)
before any work.

**Landed — `crates/ferrite-engine` (new crate, always built, no feature
flag):** `BrowserEngine` trait (`src/lib.rs`) with the directive's full
minimum action surface — `navigate`, `go_back`/`go_forward`, `reload`,
`current_url`, `dom_snapshot`, `query`, `read_text`, `click`, `type_text`,
`fill_form`, `select_option`, `scroll`, `wait_for` (`WaitCondition::
{Selector, Idle, Timeout}`), `screenshot`, `download`, `open_tab`/
`close_tab`/`switch_tab`, `cookies_read`/`storage_read` (both scoped —
take an `&Origin`), `clipboard_read`/`clipboard_write`, `js_execute`
(documented as privileged/unconstrained by this trait — the comparator's
job, per the module docs). Every method returns `Result<(T, Origin),
EngineError>`, matching the directive's "every action returns (result,
origin)" contract literally in the signature rather than as a convention.
Action-tagging reuses `ferrite_core::Primitive`'s existing wire vocabulary
(`Call::primitive()`) rather than inventing a second one — the exact
mismatch shape T-216 found is structurally avoided here, not just avoided
by discipline.

`MockEngine` (`src/mock.rs`): per-origin/per-selector scripted response
queues for `dom_snapshot`/`query`/`read_text`/`js_execute` (A6
`DryRunContent`-style: pop while >1 queued, repeat the last), real
multi-tab navigation history (`go_back`/`go_forward` truncate/replay a
`Vec<String>`), origin-scoped `cookies_read`/`storage_read` backed by
`HashMap<Origin, Vec<...>>` (proven non-leaking by test, not just by
code-reading), and a full `Vec<Call>` call log for introspection. Starts
every fresh tab at a synthetic, representable `MOCK_HOME =
"https://mock-home.ferrite.test"` rather than a real browser's opaque
`about:blank` — documented explicitly as a deliberate Mock-only
convenience; a test that wants the opaque-origin path can still navigate
`MockEngine` to a non-http(s) URL and observe `EngineError::OpaqueOrigin`.

`ferrite_engine::conformance` (feature `test-util`): three small,
engine-agnostic assertion functions (`navigate_updates_current_url_and_
origin`, `unknown_tab_id_is_a_typed_error`, `wait_idle_completes`) —
deliberately a small shared subset, not the whole suite; see below for
why a fully generic fixture-parameterized suite covering everything was
not attempted.

**Tests (`tests/conformance.rs`, 24 tests, all passing):** every one of
the 24 trait methods exercised at least once
(`every_trait_method_is_visible_in_the_call_log` asserts the call log has
exactly 24 entries after one call to each); origin-tracking asserted
through `navigate`/`go_back`/`go_forward`/`reload`
(`navigate_go_back_go_forward_and_reload_track_origin_through_history`);
`cookies_read`/`storage_read` scoping proven non-leaking
(`cookies_read_is_scoped_and_does_not_leak_other_origins_cookies`,
`storage_read_is_scoped_and_does_not_leak_other_origins_storage` — each
seeds two origins and asserts a third, unseeded origin gets nothing, not
everything); `js_execute` exercised both for success and for a scripted
failure, with an explicit test-level comment that the test proves the
method *runs*, not that it is *safe* to call (that's the comparator's
job); the opaque-origin path exercised for real
(`navigating_to_an_opaque_scheme_reports_a_typed_error_not_a_fabricated_
origin`); a `wait_for(Idle)`/`wait_for(Timeout)` pair proving no real
sleep occurs (R8) via a wall-clock `Instant` bound in the test itself.

**Landed — `crates/ferrite-engine-servo` (new crate, feature
`engine-servo` = `["ferrite-servo/servo"]`):** `ServoEngine` wraps
`ferrite_servo::session::HeadlessServoSession` per the charter's explicit
instruction (does not reimplement libservo/winit plumbing). Mapping
decisions, each documented in the crate's module docs:
- `dom_snapshot`/`query`/`read_text`/`click`/`type_text`/`select_option`
  are implemented by injecting a small `JSON.stringify(...)`-returning JS
  snippet via `execute_js` and parsing the result.
- `execute_js`'s `Ok` branch is a `Debug` rendering of Servo's JS value
  type (`format!("{:?}", v)`, in `ferrite-servo`'s existing code, not
  ours), not raw JSON. `unwrap_js_string_result` strips the observed
  `String("...")` wrapper and unescapes via `serde_json`'s string parser
  — a real, unit-tested mechanism
  (`unwrap_js_string_result_strips_the_observed_string_wrapper` et al.),
  with its dependence on that untyped channel documented plainly.
- `js_execute` itself does **not** attempt this unwrapping (arbitrary
  scripts can return any JS value shape) — returns the raw string
  verbatim, documented as the caller's own `JSON.stringify()`
  responsibility if a clean value is wanted.
- `download`/`clipboard_read`/`clipboard_write` are
  `EngineError::Unsupported` — `HeadlessServoSession` has no download
  manager and no synchronous clipboard API. Not placeholders: real,
  honest, tested "this genuinely cannot be done today" results.
- `cookies_read`/`storage_read` only succeed when `scope` equals the
  *currently loaded* page's own origin (reading `document.cookie`/
  `localStorage` via `execute_js` can only ever see that origin anyway);
  a different `scope` is `EngineError::Unsupported` rather than silently
  navigating away to satisfy it — a real scoping restriction, not a
  missing feature dressed up as one.
- Tabs are real: each open tab is its own `HeadlessServoSession`, sharing
  the process-wide Servo singleton (`ferrite-servo`'s own
  `get_or_init_servo`), mirroring `ferrite-shell`'s existing multi-tab use.

**A real, load-bearing design fix found while integrating against the
actual `servo` feature (not caught by the mock-backed default build):**
`BrowserEngine`'s original draft required `Send`. The **real**
`HeadlessServoSession` (behind the real `servo` feature) holds `Rc<...>`
state throughout — Servo's engine is an intentionally thread-affine
singleton — so `ServoEngine` cannot be `Send`, and building
`ferrite-engine-servo --features engine-servo` failed with 24 "cannot be
sent between threads safely" errors. Fixed by removing the `Send`
supertrait bound from `BrowserEngine` entirely (documented in the trait's
own doc comment, including why: no real caller in this charter's scope
needs to move a `BrowserEngine` across threads, and `MockEngine` never
needed the bound to begin with). This is exactly the class of thing the
exit gate's "attempt the real build for real" instruction exists to
surface — it would not have been found by mock-only testing.

**`just build-servo` — attempted for real, succeeded:** `cargo build -p
ferrite-shell --features ferrite-servo/servo` (the recipe's literal
body), from an `~/.cache/ferrite-target` that already held A1–A8's
non-Servo artifacts. **Wall clock: 15m 31s. Target-dir size after: 6.4
GB. Exit code: 0.** Full numbers, caveats (warm registry cache,
non-empty starting target-dir), and the comparison against the
directive's own "tens of GB, 30–60 min" unverified estimate are in
`docs/BUILD_BUDGET.md`'s new 2026-09-18 A9 section — time and disk both
landed comfortably under that estimate on this machine.

**Real-Servo `ServoEngine` conformance run — attempted, partially
succeeded, one real finding not fixed (T-220):**
`crates/ferrite-engine-servo/tests/servo_conformance.rs`, `#[ignore]`d,
run via `cargo test -p ferrite-engine-servo --features engine-servo --
--ignored --test-threads=1` against the just-built real Servo. Two
process-level blockers were found and fixed in the course of getting
this to run at all:
1. The `Send`-bound fix above (found via this exact command failing to
   compile).
2. Servo's `config::opts` module is a **process-global** singleton that
   panics ("Already initialized") if constructed twice. The first
   attempt wrote several independent `#[test]` fns (mirroring
   `ferrite-engine`'s own test-file style); this failed because Rust's
   `libtest` harness spawns a fresh OS thread per `#[test]` fn regardless
   of `--test-threads`, so `ferrite-servo`'s `thread_local!`-cached
   `get_or_init_servo()` re-initializes (and panics) on the second test
   function's thread. Fixed by consolidating into one `#[test]` fn that
   builds a single `ServoEngine` and drives every assertion through it
   sequentially on one thread — the correct, honest fix (not a workaround
   for a bug in `ServoEngine`, a real property of how the existing,
   out-of-scope-to-modify `ferrite-servo::session` code manages Servo's
   own process-wide state).

With both of those fixed, `ServoEngine::new` succeeds for real (no
panic) — but `navigate()` to a `127.0.0.1` loopback HTTP fixture server
does not observably complete: a throwaway diagnostic binary (not
committed) confirmed the fixture server never receives a TCP connection
attempt at all within the drive-until-loaded window, so `current_url()`
still reports `about:blank` afterward and the rest of the suite fails on
`EngineError::OpaqueOrigin`. This was not root-caused to a fix within
this session's remaining budget. Leading hypothesis, documented in the
test file's module docs and not confirmed: `HeadlessServoSession`'s own
doc comment says it is meant to be driven by a genuine winit
`EventLoop::run()` tick (as `ferrite-shell` does in production); bare
repeated `spin_event_loop()` calls with no real OS-level event loop
underneath may not be sufficient to make Servo's networking component
(plausibly running on its own thread/process) actually attempt a fetch.
Filed as **T-220** — real follow-up work, not a documentation gap: fixing
it means either instrumenting/modifying `ferrite-servo::session` (outside
this charter's file list) or wrapping a genuine winit loop inside
`ServoEngine` itself.

**Honest accounting against the exit gate's three parts:**
1. Full action-suite conformance vs. `MockEngine`: **done**, 24/24
   passing, part of `just test`.
2. `just build-servo` succeeds once, cost recorded: **done**, 15m31s,
   6.4 GB, recorded in `docs/BUILD_BUDGET.md`.
3. The same (or an equivalent) conformance suite run against
   `ServoEngine` at least once, result recorded: **attempted for real,
   partial result honestly recorded** — construction succeeds, real page
   navigation does not complete in this environment, root cause
   hypothesized but not fixed (T-220). Per the charter's own explicit
   allowance ("pass, fail, or 'could not attempt because the build itself
   didn't complete', whichever is true"), the true outcome here is a
   fourth case the charter didn't name outright but clearly anticipates
   the spirit of: the build *did* complete, the *engine construction*
   passes, and the *page-load leg* fails with a real, diagnosed,
   unresolved cause — recorded as exactly that, not rounded up to "pass"
   or down to "could not attempt."

**Landed — `crates/ferrite-agent/src/browser_loop.rs` (new module,
additive):** `run_agent_loop` — plan → select tool → act → observe →
repeat, generic over `&dyn BrowserEngine` and `&dyn
ferrite_model::ModelProvider` (never a concrete provider type, matching
A4's `fingerprint::generate_fingerprint` precedent and §10.5's rule).
`AgentAction` (serde-tagged enum, one variant per `BrowserEngine` method
plus `Finish`) is the model's structured-output vocabulary; the model's
raw response text is parsed directly via `serde_json::from_str` (kept
deliberately simple — no schema/cache/decorator wiring in this loop, see
the module's explicit scope note on why the old `BrowserTool`/
`AgentRuntime`/`ToolExecutor` path is untouched). `LoopBudget{max_steps,
max_wall_clock, max_repeated_identical}`, all three enforced; wall-clock
uses an injected `ferrite_core::Clock` (never `tokio::time::sleep`/real
sleep — R8). The repeated-identical-action guard fires **before**
executing the would-be-Nth-in-a-row action, so a real engine (not just
`MockEngine`) never actually performs the repeat.

**Decision — `BrowserTool`/`AgentRuntime`/`ToolExecutor`/`GeminiAgent`:
kept exactly as-is, not migrated, not removed.** Checked via `grep -rn
"BrowserTool\|AgentRuntime\|ToolExecutor\|GeminiAgent" crates/` before
deciding (per the charter's own instruction): these are live,
load-bearing infrastructure for `ferrite-ipi::dry_run` (`executor.rs`,
`orchestrator.rs`), `ferrite-ipi::tool_decision`, `ferrite-eval`
(`harness.rs`, `corpus.rs`, `tests/pilot_w6.rs`), and `ferrite-ui`/
`ferrite-shell` (the actual running agent), not dead reference material.
Rewriting that whole path onto `BrowserEngine` is real, larger work the
directive's own A9 text scopes out explicitly ("does not need to wire in
the full IPI defense end-to-end... if it's not small, say so plainly").
`browser_loop` is therefore new, additive capability living alongside the
old path, not a replacement for it.

**Tests (`browser_loop::tests`, 8 tests, all passing):**
`the_loop_stops_when_the_model_finishes`, `step_budget_is_enforced`,
`wall_clock_budget_is_enforced_with_an_injected_clock_never_real_sleep`
(asserts `Instant::elapsed() < 2s` around a call that reports a 120s
budget exhausted — proves no real sleep occurred),
`repeated_identical_actions_trigger_a_hard_stop_not_an_infinite_loop`
(scripted `MockProvider` always returns the same click; loop stops after
2 executed + 1 caught-before-execution, not an infinite loop),
`a_model_error_stops_the_loop_rather_than_panicking`,
`a_malformed_action_stops_the_loop_rather_than_panicking`,
`executed_actions_carry_real_origin_tracking_through_observations`.

**Workspace wiring:** root `Cargo.toml` — `crates/ferrite-engine` and
`crates/ferrite-engine-servo` added to `members`, plus
`[workspace.dependencies]` path entries. `ferrite-agent/Cargo.toml`
gained `ferrite-core`, `ferrite-engine`, `ferrite-model`, `chrono` — all
already-workspace deps, no new external crate added to
`[workspace.dependencies]` for this charter's own code (the `engine`
crate itself needs only `ferrite-core`/`serde`/`thiserror`, all
pre-existing).

**Verified:** `cargo test -p ferrite-engine` (24 passed, 0 failed) +
`-p ferrite-engine-servo` (5 passed, 0 failed, default features — the
`#[ignore]`d real-Servo test is 0-collected under default features via
`#![cfg(feature = "engine-servo")]`) + `-p ferrite-agent` (11 passed —
3 pre-existing `tests::*` + 8 new `browser_loop::tests::*` — 0 failed).
`cargo fmt -p ferrite-engine -p ferrite-engine-servo -p ferrite-agent
--check` clean. `cargo clippy -p ferrite-engine --all-targets -D
warnings`, `-p ferrite-engine-servo --all-targets -D warnings` (default
features), `-p ferrite-agent --all-targets -D warnings` all clean.
`cargo clippy -p ferrite-engine-servo --all-targets --features
engine-servo -- -D warnings` (the real feature) also run — see this
session's exact result recorded alongside the handoff, since it
completed after this entry was drafted. `just check` (fmt-check +
clippy --all-targets -D warnings workspace-wide + cargo machete) green —
machete initially flagged an actually-unused `serde_json` dependency in
`ferrite-engine`'s `Cargo.toml` (added speculatively, never called),
removed, machete clean after. `just test` (full workspace, default
features, isolated `CARGO_TARGET_DIR` to avoid racing the concurrent
`just build-servo` run) — every crate's `test result: ok`, 0 failures,
exit 0 (`ferrite-core` 166, `ferrite-model` 151+3 conformance,
`ferrite-ipi` 58, `ferrite-audit-log` 21, `ferrite-agent` 11,
`ferrite-engine` 24, `ferrite-engine-servo` 5, others as before).

**Known issues discovered / filed:**
- **T-220** (this session, unowned): `ServoEngine::navigate` does not
  observably complete a real page load in this environment — see above.
- **T-221** (this session, feeds A13): `ferrite-ipi`'s `Cargo.toml` still
  depends on `ferrite-agent` (`ferrite-agent = { workspace = true }`,
  confirmed present), backwards from the target dependency direction.
  Not fixed here — reversing it means relocating `BrowserTool`/
  `AgentRuntime`/`ToolExecutor` to a crate both `ipi` and `agent` can
  depend on, well beyond this charter's file list; flagged for A13 per
  the charter's own suggestion.

**Exact next action for A10:** `crates/ferrite-ui` — Iced shell with a
consent panel rendered fully decoupled from page content, plain-English
per-item diff summaries citing primitive/origin/capability, per-item
approve/reject (no blanket approve), dry-run evidence on expand,
non-sticky per-task approvals. See `docs/handoffs/a09.md` for the exact
shape of `BrowserEngine`/`AgentAction`/`AgentLoopResult` A10's UI will
need to consume.
