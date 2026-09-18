# Ferrite — Task Ledger

Stable `T-###` IDs. IDs are never reused or renumbered — a dropped task
keeps its ID with `dropped <reason>`. Derived by Agent A0 from three
sources, per `docs/REBUILD_DIRECTIVE.md` §6/A0:

1. **D1–D14** — the defect register (`docs/REBUILD_DIRECTIVE.md` §9),
   T-001 through T-014, one-to-one.
2. **A1–A13** — the agent charters (`docs/REBUILD_DIRECTIVE.md` §6),
   T-101 through T-113, one-to-one.
3. **The old `docs/archive/TO-DO.md`'s still-open items** — T-201 and up.
   Everything from that 5,909-line file was reviewed; completed work
   (Tasks 1–17, 20a–20d, W1–W6, Phase 0) is not carried forward as a task
   (it's done — see `docs/archive/PROGRESS.md` for the record, and
   `docs/DECISIONS.md` for what of it became a live ADR). What was
   genuinely still open gets a T-2xx entry below, or an explicit
   `dropped <reason>` if the rebuild directive supersedes it.

Status enum: `open` / `in-progress` / `done <SHA>` / `dropped <reason>` /
`held <reason>`.

---

## Defect register (D1–D14 → T-001–T-014)

| ID | Status | Owning charter | Defect | Fix |
|---|---|---|---|---|
| T-001 | open | A7 | `compare()` takes one global `OriginScope` for the whole task; per-capability scoping from the design docs (ADR-004) is inexpressible | New `compare()` contract: each capability in the fingerprint carries its own `OriginScope`; lowering yields `Vec<(Primitive, &OriginScope, CapabilityId)>` |
| T-002 | open | A7 | `OriginScope::admission_rank()` is computed and never consumed — dead code | Comparator attributes each event to the highest-admission-rank capability; a test asserts `admission_rank` is actually called |
| T-003 | open | A5 | Sanitizer excision built, tested, off everywhere (`strip_enabled` never set `true`) → `On` and `LoopOnly` are behaviorally identical in production despite being presented as distinct conditions | Wire excision live behind config, gated on a measured benign false-strip rate (see T-203) |
| T-004 | open | A12 | `final_outcome` in `On` mode ignores `sanitizer_caught`; once excision is live, a stripped attack reports as `Executed` — inverting the headline metric | New outcome lattice: `Stripped` / `ContainedViaConsent` / `Executed` / `NotAttempted`, table-driven test over every `(caught, gated)` pair. Depends on T-003. |
| T-005 | open | A8 | Audit hash omits `capability`/`url`; `exec_id`/`case_id` payload can be rewritten with `verify_chain()` still passing | Hash the full canonical, versioned entry payload; domain-separated preimage (`b"ferrite-audit-v1"` prefix) |
| T-006 | open | A11 | `CarrierVector` partition validated only at JSON-corpus-load time; hand-built Rust literals bypass it | Move partition enforcement into the type system (sealed constructor or per-carrier enums) |
| T-007 | open | A6 | `intercept_request()` unreachable outside its own test — containment is by mock (RecordingExecutor never makes real calls), not by interception | Default: delete the netns/interception path, state plainly "containment is by construction: the dry-run executor has no network backend." Keep only if a real-executor backend is actually planned near-term, with a test exercising it. |
| T-008 | open | A6 | Twin AES key is a compiled-in constant (`ferrite-ipi-twin-dev-key-32byte!`) | OS keyring, env fallback, fail loudly if neither present — no constant |
| T-009 | open | A11 | AgentDojo adapter is enum plumbing only (`Tier3AgentDojo`, `RunLabel::R9`), no Slack-suite mapping exists | Implement the mapping for real, or delete the enum variants — no placeholder left standing |
| T-010 | open | A12 | Simulated consent user hard-pinned to `RejectFlagged` | Make it a swept parameter (`RejectFlagged` / `ApproveAll` / `RandomP(p)`); report `RejectFlagged` explicitly as an upper bound on human vigilance, not an estimate |
| T-011 | done `01c3f54` | A0 | `CLAUDE.md` listed five completed tasks as "not yet implemented" (commit `46b2177` built them and edited the same file without fixing the claim) | Rewrote CLAUDE.md to the zero-status contract; `scripts/check_no_archive_links.sh` now fails CI if a status heading reappears |
| T-012 | done `a45b2ad`+`01c3f54`+`6ccfdad` | A0 | `.rules`/`README.md`/`commands.md`/`devcontainer.json` described the dead broker architecture; `.rules` is auto-ingested by AI tools — self-inflicted context poisoning | Deleted `.rules`/`commands.md`; rewrote `README.md`, `devcontainer.json`, `Dockerfile` |
| T-013 | done `af8d8e3` | A1 | Docs assume Windows/PowerShell/single dev machine; reality is multi-OS, multi-contributor | `justfile` added as the only documented command surface (OS-neutral, verified: `just check`/`just disk`/`just test-fast`/`just eval`/`just install-hooks` all actually run on this machine) |
| T-014 | done `9e3f097`+`ffe3c63` | A0 | `paper/PAPER_STATUS.md` tracks past deadlines against a track the engineering TO-DO says is parked | Archived to `docs/archive/paper/` with a banner noting the deadlines are stale and the track is parked |

---

## Agent charters (A1–A13 → T-101–T-113)

One row per charter. Each charter's own exit gate (`docs/REBUILD_DIRECTIVE.md`
§6) is the completion criterion — not repeated here in full.

| ID | Status | Charter | One-line scope |
|---|---|---|---|
| T-101 | done `af8d8e3`+`8b9d665`+`9fee225` | A1 — Foundation | `[workspace.dependencies]` (`8b9d665`... actually `chore(build): consolidate` commit), build profiles, `justfile`, `deny.toml`, CI matrix rework (Servo weekly not every push, widened to ubuntu/macos/windows) — done. Not done: `.cargo/config.toml`'s target-dir deliberately left to the justfile instead (documented deviation, see that file); `cargo-bloat`/`cargo-sweep` not installed (recipes exist, tools aren't — low priority, install on first real need). |
| T-102 | open | A2 — Core contracts | `ferrite-core`: capability/primitive taxonomy, newtype IDs, per-capability `OriginScope`, `Clock`, error enums, config. Feeds T-001. |
| T-103 | open | A3 — Model layer | `ferrite-model`: `ModelProvider` trait, Mock/Replay/Ollama/Gemini backends, cache+throttle+budget decorators per §10 |
| T-104 | open | A4 — Fingerprint engine | Rebuild the hybrid engine with a structurally-unrepresentable `js.execute` in `ExpectedCapability` |
| T-105 | open | A5 — Sanitizer | Pattern set as versioned data; live excision wiring. Closes T-003. |
| T-106 | open | A6 — Dry-run + twin + containment | Ordered event log with seq; keyring-backed twin key (closes T-008); decide/execute the containment path (closes T-007) |
| T-107 | open | A7 — Comparator + origin model | Per-capability scoping, admission-rank attribution. Closes T-001, T-002. Also absorbs the `compare()` double-flag de-dup (T-206). |
| T-108 | open | A8 — Audit log | Full-payload hash preimage, tamper matrix. Closes T-005. |
| T-109 | open | A9 — Engine + agent action surface | `BrowserEngine` trait, `MockEngine` + `ServoEngine`, full action surface, engine/provider-agnostic agent loop |
| T-110 | open | A10 — UI/UX | Iced consent panel decoupled from page content, per-item approve/reject, UI state-machine tests |
| T-111 | open | A11 — Dataset + corpus | Type-level carrier partition (closes T-006), AgentDojo mapping-or-delete (closes T-009), real corpus construction per §13.3. Absorbs T-201/T-203/T-208 below. |
| T-112 | open | A12 — Harness + metrics | 4-mode matrix in code, corrected outcome lattice (closes T-004), consent-policy sweep (closes T-010), Wilson/McNemar metrics module, `docs/EVALUATION.md`. Absorbs T-202/T-204 below. |
| T-113 | open | A13 — Reconciliation & release | Final doc-vs-test/SHA audit, `v0.1.0` tag, honest limitations section |

---

## Additional items from the old TO-DO.md, not otherwise covered (T-2xx)

| ID | Status | Feeds | Item |
|---|---|---|---|
| T-201 | open | A11 | **Corpus-authoring design document** (old Phase 1). Case anatomy, closed vocabularies, worked T1a/T1b examples, written against the *rebuilt* `CaseDefinition` (post T-006), not the pre-rebuild struct. Prerequisite for corpus construction, not optional preamble. |
| T-202 | open | A12 | **Per-case result inspector** (old Phase 2 item 1): a CLI that runs one authored case and renders its per-mode records legibly (which modes ran, each layer's caught/missed, final outcome, match against authored `ground_truth`). Distinct from the corpus-wide metrics aggregator (T-112) — this is the "how do I see my one case's output" tool, needed *during* authoring, before there's a corpus-worth of data to aggregate. |
| T-203 | open | A11 | **Schema-completeness audit before authoring at scale** (old Phase 2 item 2): walk every metric formula in `docs/EVALUATION.md` §13.2 against the *rebuilt* `ExecutionRecord` and confirm no metric needs an absent field, before corpus construction begins in earnest. ~1hr exercise; cheap insurance against re-running experiments. Also the natural place to lock the benign false-strip precision threshold that gates T-003's excision activation. |
| T-204 | held — per prior explicit user instruction ("not now") | A12/A13 | **Freeze the defense and run the full R1–R10 / A1–A5 experiment matrix** (old Phase 6). Carries forward as a real task, not silently dropped, but must not be auto-run at any charter's exit gate without a fresh go-ahead — the hold was an explicit decision, not a scheduling gap. |
| T-205 | dropped — architecture deleted | — | Extism/Wasm extension sandbox and its demo extension (old Task 3, `extensions/hello-ext`, `ferrite-sandbox`). Deleted per `docs/REBUILD_DIRECTIVE.md` §4 (commit `a45b2ad`). `ferrite-engine`'s `MockEngine`-first design (T-109) replaces the sandboxing concern's original motivation (a safe place to run untrusted code) with a different mechanism (a safe place to run the *agent*, via the dry-run loop) — not a gap that needs separately re-filling. |
| T-206 | open — merges into T-107 | A7 | **`compare()` double-flag de-dup**: an out-of-scope-origin event whose primitive is also outside the expected union currently increments both `out_of_scope_origins` and `extra_primitives`. Predates the D1–D14 list (found in the pre-rebuild `PROGRESS.md`'s own Known Issues); recorded here so it isn't lost, resolved as part of T-107's rewrite of the same function rather than as a standalone patch to code being replaced anyway. |
| T-207 | done `42401fd`+`068fd8e` | A1 | **Workspace `cargo fmt`/`clippy --all-targets` gap — fully closed.** Fixed via `cargo fmt --all` (whitespace only, zero behavior change) plus documented `#[allow(clippy::await_holding_lock)]` on the 10 `ENV_GUARD`-across-`.await` test call sites in `ferrite-eval` (real root cause: the OLD CI's `cargo clippy --workspace` lacked `--all-targets` and so never linted test code at all — now fixed in `.github/workflows/ci.yml`, T-101). Verified: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` both clean workspace-wide, `cargo test --workspace` green throughout. |
| T-208 | open | A11 | **Pilot corpus is a real, reusable seed — not grandfathered.** The 10 cases in `crates/ferrite-eval/tests/pilot_corpus/` (plus `CORPUS_AUTHORING_GUIDE.md`/`PILOT_CORPUS_REFERENCE.md`) should seed the real corpus, but every case must be: (a) re-labelled under the post-rebuild `GroundTruth` enum as it exists once T-006/T-111 land, (b) re-validated against the type-level carrier partition (T-006), and (c) counted toward the 10% double-authoring / Cohen's κ ≥ 0.8 requirement (§13.3) like any newly authored case — no exemption for being pre-existing. |

---

| T-209 | **needs owner confirmation** | — | A1 added `license = "MIT OR Apache-2.0"` and `publish = false` to all 7 crate manifests (commit `8b9d665`) because `cargo deny check licenses` requires a license field and none existed. The dual-license choice came from the old (now-archived) `.rules` file's stated intent ("License: Dual MIT/Apache-2.0"), not a fresh decision this session made or confirmed with the project owner. If that's not the intended license, change it before this is ever published anywhere — `publish = false` means it can't reach crates.io by accident either way, but the LICENSE-MIT/LICENSE-APACHE files a real dual-license claim implies don't exist yet. |
| T-210 | **needs owner confirmation** | A1 | CI's Servo-enabled "latest" release moved from every push to main → weekly (Monday 06:00 UTC) + manual `workflow_dispatch` (commit `9fee225`), per `docs/REBUILD_DIRECTIVE.md` §7.1's Servo-is-expensive mandate. This is a real, visible behavior change to something the pre-rebuild team explicitly built (a dedicated "Job 2: Publish a rolling latest release" pipeline) — not committed to a remote yet (10+ commits sit local, unpushed, per `docs/PROGRESS.md`), so nothing has broken for anyone yet, but flagging before it does. |

## Notes for whoever picks up A1 next

- The workspace was flattened from `Browser/*` to the repo root in A0
  (commit `e474403`), proven content-identical via `cargo metadata`
  before/after. If you find a stale `Browser/`-prefixed path anywhere,
  that's residue, not intentional.
- `docs/ARCHITECTURE.md` does not exist yet. `CLAUDE.md` points to it as
  "not yet written." A1/A2 is the natural place for it to appear, per
  `docs/REBUILD_DIRECTIVE.md` §12's completeness check (deferred until the
  file exists — see `scripts/check_no_archive_links.sh`'s comments).
- `scripts/check_purge.sh` and `scripts/check_no_archive_links.sh` both
  pass clean as of A0's last commit. Wire them into CI as part of T-101 —
  they're currently run-by-hand only.
