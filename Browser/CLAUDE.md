# Ferrite Browser — Claude Code Context

> IPI-defended, audit-logged agentic browser in Rust, built on Servo.
> This file is the authoritative coding-rules reference. When it conflicts with
> older comments or docs, this file wins. Update it when architecture changes.

## MANDATORY: Progress Tracking

Every change to this codebase requires a corresponding update to `PROGRESS.md`
(workspace root). No exceptions — new features, dependency changes, bug fixes,
files created/moved/deleted, milestones reached. Add a dated entry under the
Change Log describing exactly what changed, and update the Milestone Status
table. Err toward more detail; future sessions depend on this file.

## What Ferrite Is

A developer-focused Rust browser that treats AI agents as first-class principals
governed by an architectural (not model-level) defense against Indirect Prompt
Injection (IPI). The IPI defense system (`ferrite-ipi`) is the primary system and
contribution; the hash-chained audit log and capability-lineage thinking are
supporting pillars, not the headline.

The strategic principle behind every design decision: solve security
architecturally, not by competing on model training. Prefer enforcement that
works regardless of what the underlying LLM does.

## Active Workspace (ground truth)

Seven crates. This is the complete, current set — verified against the workspace
manifest. Do not reference or import any crate not in this list.

| Crate | Role |
|-------|------|
| `ferrite-shell` | Top-level binary; CLI arg dispatch; smoke tests |
| `ferrite-servo` | Servo rendering integration (`HeadlessServoSession`) |
| `ferrite-ui` | Iced dark-mode UI; collapsible agent sidebar (320px, right) |
| `ferrite-audit-log` | `PersistentAuditLog`: SHA-256 hash chain + SQLite (rusqlite) |
| `ferrite-agent` | `BrowserToolExecutor`, `GeminiAgent`, `AgentRuntime` trait, rate limiter |
| `ferrite-ipi` | Seven-component IPI defense system (see below) |
| `ferrite-eval` | Evaluation harness (Servo-free; depends only on `ferrite-ipi` + `ferrite-agent`). Locally buildable by design — relieves the CI-only build bottleneck. |

> `ferrite-eval` is the evaluation harness home (EVALUATION_PLAN §8). It is Servo-free
> so it builds and runs without the heavy Servo toolchain. If a future evaluation case
> genuinely needs a real page fetch, that goes behind an optional Servo-backed executor
> feature flag — the default eval path stays Servo-free.

## Deferred Components (dormant, NOT deleted)

The following were removed from the active workspace but remain on disk and may
be re-integrated in a later stage. Do NOT add them back to the workspace, import
them, reference them in new code, or propose architecture that depends on them —
unless explicitly instructed. Treat them as out of scope for current work:

- Capability broker (`ferrite-capability-broker`)
- Policy engine / Regorus (`ferrite-policy`)
- Wasm/Extism extension sandbox (`ferrite-sandbox`)
- CEF / Track B engine harness
- adblock-rust content blocking
- Accessibility (AX) tree extraction

If a task seems to need one of these, stop and flag it rather than reviving the crate.

## ferrite-ipi: Seven-Component Architecture

1. Hybrid tool-decision engine — rule-based must-use + LLM may-use (temperature 0); the two sets are kept strictly disjoint
2. HTML/JS sanitizer
3. Synthetic data twin — AES-256-GCM, TTL rotation
4. Dual-layer network containment — Tokio/hyper interceptor (all platforms) + Linux network namespace (Linux-only, via `nix`)
5. Dry-run `RecordingExecutor`
6. Fingerprint comparator + Iced consent UI
7. IPI adversarial dataset pipeline

Module layout is flat files under `crates/ferrite-ipi/src/` (e.g. `comparator.rs`,
`dataset.rs`) — NOT `module/mod.rs` subdirectories, EXCEPT `tool_decision/` which is
already a directory module (`tool_decision/mod.rs`). Match the existing layout when
editing; do not create a subdirectory for a module that already exists as a flat file,
and do not flatten one that already exists as a directory.

Behavioral notes that are easy to get wrong:
- Open-ended / vague prompts correctly produce empty fingerprints. This is intended, not a bug.
- Fingerprints accumulate across turns within a session.
- The system assumes reasonably specific user prompts.
- The LLM may-use predictor returns an empty set on any error (fail-safe), and
  filters responses against an allowlist so the model cannot inject arbitrary tool IDs.

## Tool Vocabulary and Capability Model (AUTHORITATIVE — source of truth)

> This section is the canonical tool vocabulary. The rule engine (`rule_based_must_use`),
> the LLM predictor allowlist, the comparator, and the dataset schema MUST obey it. It
> resolves the historical vocabulary mismatch (fingerprints emitted semantic tool IDs that
> no `BrowserTool` could produce). Full rationale: `FINALIZED_DECISIONS.md` Decisions 1–6.

### The eight primitives (the ONLY real tool IDs)

`BrowserTool`'s nine variants collapse to eight distinct primitive IDs. These are the only
strings the dry-run can ever record, and therefore the only strings the comparator compares:

| `BrowserTool` variant | primitive id |
|---|---|
| `Navigate(String)` | `navigate` |
| `ReadPage` / `ExtractData(String)` | `dom.read` |
| `ClickElement(String)` | `dom.write` |
| `FillForm { selector, value }` | `form.fill` |
| `ReadClipboard` | `clipboard.read` |
| `WriteClipboard(String)` | `clipboard.write` |
| `ExecuteJs(String)` | `js.execute` |
| `DownloadFile(String)` | `download.file` |

Any tool ID outside these eight is a PHANTOM and must never appear in a fingerprint.
**Cut entirely** (no primitive realizes them): `email.read/send/draft`, `calendar.read/write`,
`form.submit`, `report.write`, `contacts.read`, `storage.read/write`, `network.fetch`,
`screenshot`. (`form.submit` does not exist — `FillForm` → `form.fill` only; model a submit as
an `interact` action. `report.write` is the agent producing output, not a browser action.
`network.fetch` is not a tool — network reachability is the *origin* dimension, checked via
origin scope, never via a tool's presence.)

### Six action classes (group primitives by security character)

| Action class | primitives | character |
|---|---|---|
| `read` | `dom.read` | observe page content (passive) |
| `interact` | `dom.write`, `form.fill` | modify page / enter data (active, on-page) |
| `navigate` | `navigate` | move to an origin (origin-changing) |
| `download` | `download.file` | pull a resource (origin-touching) |
| `clipboard` | `clipboard.read`, `clipboard.write` | local side channel (no origin) |
| `execute` | `js.execute` | arbitrary code — **UNSCOPABLE** |

### Capabilities = (action class × origin scope)

A capability is an action class paired with an origin scope. The semantic *name* is a human
label for consent UI and fingerprints; it carries no hidden tool. There is NO `email.read`
capability — "email" is a property of the origin scope, authored per task, not a fake tool.

| Capability (label) | action classes | origin scope |
|---|---|---|
| `web.read` | read, navigate | task-declared origin(s) |
| `web.interact` | interact, navigate | task-declared origin(s) |
| `web.download` | download, navigate | task-declared origin(s) |
| `scoped.read` | read, navigate | a NARROW declared origin class (the "email.read"-style tight capability) |
| `clipboard.read` | clipboard | none (local) |
| `clipboard.write` | clipboard | none (local) |

### The `unscopable` rule (js.execute)

`execute` (sole member `js.execute`) is **unscopable**: it can impersonate any other primitive
invisibly, past the `ToolExecutor` boundary. It is NEVER part of any capability's expected
realization. The comparator applies a GENERAL rule — *any actual primitive whose action class
is unscopable is unconditionally an `extra_primitive`* — so `js.execute` is always a deviation
and always consent-gated. Encode this as an `unscopable` property of the action class, not as a
`js.execute` magic-string special-case.

### Comparator: lower-then-compare with per-origin attribution

The fingerprint is semantic capabilities; the dry-run records primitives + origins. The
comparator **lowers** capabilities to expected (primitives + origin scopes), then compares
against actual. Deviation is **per-origin attributed**:
1. group actual actions by origin;
2. for each origin cluster, find expected capabilities whose scope admits that origin;
3. attribute to the MOST SPECIFIC admitting capability (specificity precedence:
   `exact` > `domain_suffix` > `task_open`);
4. an origin admitted by no capability → `out_of_scope_origins`;
5. a primitive outside its attributed capability's action classes → `extra_primitives`.

This requires the dry-run record to bind each primitive to the origin it acted on: replace the
lossy `tools_called: HashSet<ToolId>` with an ordered event log `Vec<{ tool, origin }>`, with a
`current_origin` tracked in `RecordingExecutor` (updated on each `Navigate`, seeded from
`AgentTask.context_url`). Concurrency is fine — recording is serialized through the record mutex.

### Closed tag vocabularies (for the dataset / corpus authoring)

**Attack technique** (multi-valued, ≥1 per attack case; orthogonal to carrier_vector):
- rhetorical: `instruction_override`, `context_manipulation`, `social_engineering`, `goal_hijack`
- concealment: `obfuscation`, `payload_splitting`, `plain`

**carrier_vector** (exactly one per case; partition must match the `carrier` field):
- `WebContent` (T1a): `hidden_element`, `offscreen_text`, `html_comment`, `alt_text`,
  `meta_content`, `css_pseudo`, `visible_text`
- `ToolOutput` (T1b): `tool_json_field`, `tool_text_blob`, `tool_error_message`, `tool_metadata`

**attack_category** (objective-primary; `in_scope` true for 1–4, false for 5):
`data_exfiltration`, `unauthorized_action`, `agent_redirection`, `scope_escalation`,
`within_fingerprint_abuse`.

### API key loading (shared loader — REQUIRED)

The Gemini API key is loaded at RUNTIME, not build time, to avoid committing it to a public
repo. There must be ONE shared loader used by BOTH `gemini.rs` and `tool_decision`:
**env var (`FERRITE_GEMINI_API_KEY`) first, then `gemini_key.txt` next to the executable as
fallback.** Do not let the two components diverge (historically `tool_decision` read env-only
and silently ran keyless — degrading the may-use layer and confounding M3). During an
EVALUATION run, emit a non-fatal WARNING (warn, never fail — rules-only is legitimate) if the
predictor initializes keyless, so degraded-predictor numbers are never silently recorded.

## ferrite-ipi: Planned Near-Term Work (not yet implemented — see TO-DO.md + EVALUATION_PLAN)

Ordered; earlier gates later. Full design in `FINALIZED_DECISIONS.md` and `EVALUATION_PLAN.md`.

1. **Pre-Task-19 vocab-fix block** (upstream of the schema): rewrite `rule_based_must_use` and
   the predictor allowlist to emit ONLY the capability vocabulary above (cut all phantoms); add
   origin-binding to the dry-run record (ordered event log); rewrite `compare()` to
   lower-then-compare with per-origin attribution + the unscopable rule; unify key loading
   (shared loader above). Contained to `tool_decision`, `dry_run.rs`, `comparator.rs`. The
   existing `comparator.rs` tests encode the old single-vocabulary model and will be rewritten.
2. **Defense mode toggle (Task 18)** — `DefenseMode { On, SanitizerOnly, Off }`. `Off` bypasses
   the entire predict→dry-run→compare→consent loop; `SanitizerOnly` runs the sanitizer but
   bypasses the loop; `On` is the unchanged default. Switchable via setter + `FERRITE_DEFENSE`
   env var. For §4 baseline + optional ablation.
3. **Component 7 `dataset.rs` (Task 19)** — implement to the finalized two-layer schema in
   `EVALUATION_PLAN.md` §7. Flat `src/dataset.rs` (not a mod dir). Supersedes the thin
   `IpiEvent`/`IpiLabel` currently in `comparator.rs`. rusqlite 0.37; temp paths via
   `std::env::temp_dir()`.
4. **Sanitizer T1b extension (Task 20)** — extend `sanitizer.rs` to scan tool-output text, not
   just HTML/JS. Extend existing pattern logic; do not create a parallel module.
5. **Evaluation harness (Task 21)** in `ferrite-eval`; **AgentDojo Slack adapter (Task 22)**.

## Environment

- OS / shell: Windows, PowerShell. No WSL2.
- Rust toolchain: stable MSVC.
- IDE: Google Antigravity (VS Code fork).
- Project path: `C:\Dev\Major Project\Browser\`.
- CI: GitHub Actions, Windows + macOS matrix. Linux is intentionally NOT in CI.
  Consequence: the Linux-only network-namespace code path (component 4) is not
  compiled by CI. Account for this — do not assume CI covers it.

## Dependency Rules

- rusqlite is `0.37` with `features = ["bundled"]`. This exact version, workspace-wide. Never add a second rusqlite version.
- Do NOT add `sea-query`, `sea-orm`, `sqlx`, or `diesel`. All SQLite access uses rusqlite directly. (Committing `Cargo.lock`, with it removed from `.gitignore`, resolved a recurring CI E0004 from `sea-query-rusqlite` — do not reintroduce that family.)
- Reuse versions already in the workspace; never introduce a duplicate version of an existing dependency: tokio "1", serde "1", uuid "1", reqwest "0.12", thiserror "1", iced "0.13".
- Before adding any dependency, mentally run `cargo tree --duplicates`. If it would duplicate an existing workspace dep, find another approach.
- Inherited Dependabot alerts transitive from Servo v0.0.5 are non-actionable until the next Servo bump. Do not attempt to patch them individually.

## Hard Verification Rule

Never propose a `Cargo.toml` edit, dependency change, or architecture claim
without first confirming the relevant crate/version/file actually exists in the
workspace as described. Stale assumptions have repeatedly caused breakage
(fabricated crates, wrong rusqlite version, nonexistent dependencies). If you
cannot verify, say so and ask — do not guess.

## Working Style

- Plan before building. Confirm understanding, then implement.
- Divide large tasks into independently executable blocks, each with a clear exit condition.
- Expect and welcome correction; verify against local state rather than memory.

## Coding Conventions

- Rust edition: per-crate as set in each Cargo.toml (mixed 2021/2024 currently exists; do not change a crate's edition without flagging it).
- `cargo fmt` before every commit; default rustfmt settings.
- `cargo clippy -- -D warnings` — zero-warnings policy.
- `thiserror` for library error types; `anyhow` only in the shell binary.
- Prefer `tracing` over `println!` for logging in library code.
- All async uses Tokio.
- No `unsafe` outside FFI boundaries (Servo integration).
- Test temp paths: use `std::env::temp_dir()`, never hardcoded `/tmp/` (Windows has no `/tmp`).

## Key Files

- `Browser/Cargo.toml` — workspace manifest (source of truth for active crates)
- `Browser/PROGRESS.md` — dated change log + milestone status
- `Browser/TO-DO.md` — task/block breakdown with exit conditions
- `Browser/CLAUDE.md` — this file
- `EVALUATION_PLAN.md` (repo root) — evaluation methodology; authoritative for the
  `dataset.rs` schema and planned eval-related code. Living/volatile document.
- `FINALIZED_DECISIONS.md` (repo root) — the resolved design decisions (vocabulary model,
  schema contract, comparator rule) this CLAUDE.md vocabulary section summarizes.