# Ferrite Browser — Progress Tracker

> **How to use this file:**
> Update this file every time a feature is implemented, a crate is modified, or a milestone is reached.
> Each entry should include the date, what changed, and what the current state is.
> This file lives in `Browser/` (the Cargo workspace root) and is tracked by git.

---

## Project Structure

```
Major Project/                  ← git repo root, reference docs
└── Browser/                    ← Cargo workspace root, all Rust code
    ├── Cargo.toml              ← workspace manifest (source of truth for active crates)
    ├── CLAUDE.md               ← Claude Code coding-rules context file
    ├── PROGRESS.md             ← this file
    └── crates/
        ├── ferrite-shell/      ← binary: browser shell, CLI dispatch, smoke tests
        ├── ferrite-servo/      ← lib: Servo rendering integration (HeadlessServoSession)
        ├── ferrite-ui/         ← lib: Iced dark-mode UI, agent sidebar
        ├── ferrite-audit-log/  ← lib: SHA-256 hash-chained log + SQLite (rusqlite 0.37)
        ├── ferrite-agent/      ← lib: BrowserToolExecutor, GeminiAgent, AgentRuntime, rate limiter
        ├── ferrite-ipi/        ← lib: seven-component IPI defense system
        └── ferrite-eval/       ← lib: evaluation harness (Servo-free; deps only ferrite-ipi + ferrite-agent)
```

> **Active workspace = the seven crates above** (verified against `Cargo.toml`).
> The capability broker, policy engine (Regorus), and Extism extension sandbox were
> removed from the active workspace on 2026-04-01. They remain dormant on disk and may
> be re-integrated in a later stage — they are **not** part of current scope. See the
> 2026-04-01 Change Log entry for the removal details.

---

## Stage Overview

- **Stage 1 — Browser infrastructure: ✅ Complete.** Servo embedding, Iced UI, persistent hash-chained audit log, and the `ferrite-agent` runtime (Gemini backend, tool executor bridge, rate limiter).
- **Stage 2 — `ferrite-ipi` defense system: ⏳ In progress.** Six of the seven components are implemented and passing (30/30 tests); the dataset pipeline (`dataset.rs`) is still a stub. Note: the evaluation-design work (see `EVALUATION_PLAN.md`, `FINALIZED_DECISIONS.md`) revealed that a **vocab-fix block must precede the dataset** — the fingerprint vocabulary, the dry-run record, and the comparator are reworked first so the dataset schema is built on a sound base. Sequence is now: vocab-fix block → defense-mode toggle (Task 18) → dataset pipeline (Task 19). See "What To Do Next."
- **Stage 2 evaluation harness — planned.** A seventh crate, `ferrite-eval`, will host the evaluation harness (Tasks 21–22). Servo-free by design.

---

## Milestone Status

| Milestone | Status |
|-----------|--------|
| Cargo workspace scaffolded | ✅ Done |
| `ferrite-audit-log` — hash chain + SQLite | ✅ Done |
| `ferrite-shell` — integration smoke test | ✅ Done |
| GitHub Actions CI pipeline (Windows + macOS) | ✅ Done |
| `ferrite-servo` crate scaffolded | ✅ Done |
| Servo embedding shell — WindowRenderingContext + WebView wired in | ✅ Done |
| Iced UI shell | ✅ Done |
| Audit Log Viewer panel in Iced UI | ✅ Done |
| Capability broker / policy engine / Extism sandbox | ⏸ Deferred (dormant on disk, removed from workspace 2026-04-01) |
| UI Polish — navigation controls, visual overhaul, keyboard shortcuts, smart URL | ✅ Done |
| UI Polish — error page, new-tab page, tab titles, favicon placeholders | ✅ Done |
| JS Console panel (`JsExecute` capability) | ✅ Done |
| Full mouse/scroll/click interactivity forwarded to Servo WebView | ✅ Done |
| Platform-aware keyboard shortcuts (macOS ⌘, Windows/Linux Ctrl) | ✅ Done |
| Smart URL resolver (no redundant https://, search fallback) | ✅ Done |
| Realistic home page with 6 quick-access tiles + shortcut reference | ✅ Done |
| `ferrite-agent` — Task 9 Block 1: types, traits, rate limiter | ✅ Done |
| `ferrite-agent::gemini` — Task 10 Block 1: GeminiAgent with function calling | ✅ Done |
| `ferrite-ui` — Task 11 Block 1: tool executor bridge (agent ↔ Iced channel) | ✅ Done |
| `ferrite-ui` — Task 11 Block 2: agent sidebar panel in Iced UI | ✅ Done |
| `ferrite-ipi` — Task 12 Block 1: crate skeleton + `ToolId` + `ToolFingerprint` | ✅ Done |
| `ferrite-ipi` — Task 12 Block 2: `rule_based_must_use` keyword matcher | ✅ Done |
| `ferrite-ipi` — Task 12 Block 3: `LlmMayUsePredictor` Gemini-backed may-use predictor | ✅ Done |
| `ferrite-ipi` — Task 12 Block 4: `ToolDecisionEngine` composing both layers | ✅ Done |
| `ferrite-ipi` — Task 13 Block 1: HTML/JS sanitizer (`sanitizer` module) | ✅ Done |
| `ferrite-ipi` — Task 14 Block 1: `SyntheticTwin` + AES-256-GCM encryption + TTL rotation | ✅ Done |
| `ferrite-ipi` — Task 15 Block 1: network containment (`ContainmentState` + Option C interceptor + Option B namespace) | ✅ Done |
| `ferrite-ipi` — Task 16 Block 1: `DryRunRecord` + `RecordingExecutor` + `DryRunOrchestrator::run()` | ✅ Done |
| `ferrite-ipi` — Task 17 Block 1: `FingerprintDiff`, `compare()`, `ConsentDecision`, `IpiEvent` | ✅ Done |
| `ferrite-ui` — Task 17 Block 2: IPI consent panel in agent sidebar + dry run wired to `AgentTaskSubmitted` | ✅ Done |
| Pre-Task-19 vocab-fix block: capability vocab + origin-bound dry-run + comparator rewrite + shared key-loader | ⬜ Not started |
| `ferrite-ipi` — Task 18: defense-mode toggle (`DefenseMode { On, SanitizerOnly, Off }`) | ⬜ Not started |
| `ferrite-ipi` — Task 19: dataset pipeline (`dataset.rs`) — schema per EVALUATION_PLAN §7 | ⬜ Not started (stub) |
| `ferrite-ipi` — Task 20: sanitizer T1b extension | ⬜ Not started |
| `ferrite-eval` — Task 21: evaluation harness | ⬜ Not started |
| `ferrite-eval` — Task 22: AgentDojo Slack adapter | ⬜ Not started |
| `ferrite-agent::gemini` — `read_api_key()` free fn + `GeminiAgent::from_key()` constructor | ✅ Done |
| `ferrite-ui` — `AgentTaskSubmitted` uses `read_api_key()` + `from_key()` instead of raw env check | ✅ Done |

---

## Change Log

### 2026-06-27 — Evaluation design frozen; vocabulary + schema decisions recorded

No code changed. Recording that the evaluation-design phase is complete and its decisions
are now in the canonical docs:
- `EVALUATION_PLAN.md` — finalized §7 dataset schema (two-layer CaseDefinition + ExecutionRecord,
  GroundTruth enum), scope-tightness stratification in §5/§9, resolved-decisions record in §10.
- `CLAUDE.md` — new authoritative *Tool Vocabulary and Capability Model* section (eight primitives,
  six action classes, capability model, phantom-cut list, technique + carrier_vector closed
  vocabularies, the `unscopable` js.execute rule, shared key-loader requirement); workspace
  corrected to seven crates (added `ferrite-eval`); ON/OFF toggle corrected to three-mode.
- `FINALIZED_DECISIONS.md` (repo root) — new file; Decisions 1–6 with rationale (capability
  mapping, technique vocab, js.execute encoding, origin-scope authoring, schema field contract,
  carrier_vector + GroundTruth). Also logs the key-loading divergence finding.
- `TO-DO.md` — renumbered: Task 18 = defense-mode toggle, Task 19 = dataset, Task 20 = sanitizer
  T1b, Task 21 = eval harness, Task 22 = AgentDojo adapter; vocab-fix block precedes Task 19.

Net effect on plan: a pre-Task-19 vocab-fix block (capability vocabulary + origin-bound dry-run
record + lower-then-compare comparator + shared key-loader) is now the next implementation step,
upstream of the dataset. No design decisions remain open.

### 2026-05-04 — `read_api_key()` + `GeminiAgent::from_key()` + ferrite-ui wired up

**Files:**
- `crates/ferrite-agent/src/gemini.rs` — added `pub fn read_api_key() -> Result<String, String>` above `impl GeminiAgent`; tries env var first, then `gemini_key.txt` next to exe, then returns a descriptive error with both options; added `pub fn from_key(api_key: impl Into<String>) -> Self`; `from_env()` now delegates to both
- `crates/ferrite-ui/src/lib.rs` — `AgentTaskSubmitted` handler replaced raw `std::env::var` check + `GeminiAgent::from_env()` with `ferrite_agent::gemini::read_api_key()` match + `GeminiAgent::from_key(api_key)`; user now sees the full "create gemini_key.txt or set env var" message instead of a terse "not set" error

### 2026-04-14 — Task 17 Block 2: IPI consent panel + dry run wired to agent sidebar

**Files:**
- `crates/ferrite-ui/Cargo.toml` — added `ferrite-ipi = { path = "../ferrite-ipi" }`
- `crates/ferrite-ui/src/lib.rs` — multiple additions

**State additions:** `pending_diff: Option<FingerprintDiff>`, `pending_decision: ConsentDecision`, `approved_extras: HashSet<ToolId>`, `pending_task: Option<AgentTask>`

**New message variants:** `ConsentRequired(FingerprintDiff)`, `ApproveTool(String)`, `RejectTool(String)`, `ConsentSubmitted`, `ConsentCancelled`

**`FilteredToolExecutor`** — wraps `BrowserToolExecutor`; blocks any tool whose `ToolId` is in the `rejected` set with `AgentToolResult::err("blocked by user consent")`

**`AgentTaskSubmitted` modified** — runs IPI dry run before real agent run: creates `ToolDecisionEngine`, calls `fingerprint_from_task`, then `DryRunOrchestrator::run`; sends `AgentToolLogged("[dry run complete…]")` on completion; if `diff.is_clean()` proceeds directly to real run, otherwise sends `ConsentRequired(diff)` and exits the task

**`run_agent_loop` helper** — extracted the turn loop so it can be reused from both the direct path and `ConsentSubmitted`

**`ConsentSubmitted` handler** — clones rejected set, sets `approved_extras`, spawns new task with `FilteredToolExecutor` wrapping the real executor

**Consent panel in `view_agent_sidebar()`** — amber-tinted scrollable panel (C_WARN, 0.08α) with: danger-red header, `diff.summary()`, alphabetically-sorted tool rows with Approve (green, highlighted when approved) / Reject (red, highlighted when rejected) buttons, "Proceed with approved" (disabled until `is_complete(diff)`), and Cancel button; replaces tool log + response when `pending_diff.is_some()`

`cargo build -p ferrite-ui` — clean, no warnings

### 2026-04-14 — Task 17 Block 1: `FingerprintDiff`, `compare()`, `ConsentDecision`, `IpiEvent`

**Files:**
- `crates/ferrite-ipi/src/comparator.rs` — full implementation (no new deps needed)

- `FingerprintDiff` — holds `extra_tools` and `extra_origins` as `HashSet`; `is_clean()` checks both empty; `summary()` builds a human-readable consent-dialog string
- `compare(expected, actual)` — iterates `actual.tools_called`, keeps tools not in `expected.contains()`; origins only flagged when `network.fetch` is absent from the expected fingerprint
- `ConsentDecision` — `approve`/`reject` maintain two disjoint sets; `is_complete(diff)` checks every extra tool has a decision
- `IpiEvent` + `IpiLabel` — serialisable event type for the dataset pipeline
- 6/6 new comparator tests pass; 30/30 total `ferrite-ipi` tests pass

### 2026-04-14 — Task 16 Block 1: `DryRunRecord` + `RecordingExecutor` + `DryRunOrchestrator::run()`

**Files:**
- `crates/ferrite-ipi/Cargo.toml` — added `async-trait = "0.1"`
- `crates/ferrite-ipi/src/dry_run.rs` — full implementation

- `DryRunRecord` — accumulates `tools_called` (HashSet<ToolId>), `origins_touched`, `data_fields_accessed`, `network_attempts`, and `completed` flag; `record_network_attempt` extracts and stores the origin via `url::Url::parse`
- `RecordingExecutor` (private) — implements `ToolExecutor`; records every tool call by `ToolId`; records navigate URLs as network attempts; returns synthetic twin data for `ReadPage` and `ExtractData`; never touches Servo
- `DryRunOrchestrator::run<R: AgentRuntime>` — calls `activate_full` (Option C + B), wraps agent turn in `tokio::time::timeout(30s)`, always calls `deactivate`, harvests any containment-intercepted URLs into the record, sets `completed = true` on success, returns partial record on timeout
- Test `/tmp/` paths replaced with `std::env::temp_dir()` for Windows compatibility
- 2/2 new dry_run tests pass; 24/24 total `ferrite-ipi` tests pass

### 2026-04-14 — Task 15 Block 1: network containment — Option C interceptor + Option B namespace

**Files:**
- `crates/ferrite-ipi/Cargo.toml` — added `url = "2"`; linux-only `nix = "0.28"` (features: net, user)
- `crates/ferrite-ipi/src/containment.rs` — full implementation

- `ContainmentState` / `SharedContainmentState` (`Arc<Mutex<ContainmentState>>`) — holds `active` flag and `intercepted_urls` log
- `activate` / `deactivate` — flip the flag and clear/preserve the URL log
- `intercept_request(state, url, twin)` — returns `None` when inactive; when active, appends the URL and returns a JSON fake response carrying the synthetic twin name
- `intercepted_urls(state)` — snapshot of caught URLs
- `create_network_namespace()` — on Linux calls `nix::sched::unshare(CLONE_NEWNET)`; on all other platforms returns `Ok(())` immediately (no-op)
- `activate_full(state)` — runs Option C then Option B
- 3/3 new containment tests pass; 22/22 total `ferrite-ipi` tests pass

### 2026-04-14 — Task 14 Block 1: `SyntheticTwin` — AES-256-GCM encryption + TTL rotation

**Files:**
- `crates/ferrite-ipi/Cargo.toml` — added `aes-gcm` (0.10), `rand` (0.8), `chrono` (0.4, serde feature)
- `crates/ferrite-ipi/src/twin.rs` — full implementation

- `SyntheticTwin` struct — serializable, holds name/email/password/phone/credit_card/ssn/address/created_at
- `SyntheticTwin::generate()` — random 4-digit ID produces plausible but entirely fictitious values; all data points to `ferrite-test.invalid` or obviously fake numbers
- `SyntheticTwin::is_expired(ttl_hours)` — compares age via `chrono::Utc::now()`
- `encrypt_twin` / `decrypt_twin` — AES-256-GCM with fixed dev key; 12-byte random nonce prepended to ciphertext; JSON serialisation via `serde_json`
- `TwinManager::new(path)` / `load_or_generate()` — reads+decrypts from disk if present and unexpired; otherwise generates fresh twin, encrypts, writes to disk
- Test `/tmp/` path replaced with `std::env::temp_dir()` for Windows compatibility
- 4/4 new twin tests pass; 19/19 total `ferrite-ipi` tests pass

### 2026-04-14 — Task 13 Block 1: HTML/JS sanitizer

**Files:**
- `crates/ferrite-ipi/Cargo.toml` — added `ammonia` (3), `regex` (1), `sha2` (0.10), `hex` (0.4)
- `crates/ferrite-ipi/src/sanitizer.rs` — full implementation

- `SanitizedPage` struct: `clean_html`, `extracted_scripts`, `raw_html_hash`
- `sanitize_html(raw_html)` — extracts `<script>` content with regex before stripping, computes SHA-256 of raw HTML, then runs ammonia with strict tag/attribute allowlist (strips scripts, style, iframe, object, embed and all event handlers; allows only https/http URL schemes)
- `detect_js_injection_patterns(js)` — scans extracted JS for 8 patterns: instruction override, system prompt reference, data exfiltration language, fetch(), WebSocket, document.cookie, localStorage/sessionStorage, sendBeacon
- `sha256_hex(data)` — pure-Rust SHA-256 via `sha2` crate, hex-encoded
- 5/5 new sanitizer tests pass; 15/15 total `ferrite-ipi` tests pass

### 2026-04-14 — Task 12 Block 4: `ToolDecisionEngine` — composing both layers

**Files:** `crates/ferrite-ipi/src/tool_decision/mod.rs`

- Added `ToolDecisionEngine` struct wrapping `Option<LlmMayUsePredictor>`
- `new()` calls `LlmMayUsePredictor::from_env()` — no key → LLM layer silently disabled
- `generate_fingerprint(prompt, task_id)` → runs rule-based must-use, then LLM may-use (if predictor present), then filters may-use to keep sets strictly disjoint
- `fingerprint_from_task(task)` → convenience wrapper for `AgentTask`
- `Default` impl delegates to `new()`
- 3 new async tests in `engine_tests` (all pass without API key): `engine_no_api_key_uses_rules_only`, `engine_open_ended_prompt_produces_empty_fingerprint`, `must_use_and_may_use_are_disjoint`
- 10/10 tests pass

### 2026-04-14 — Task 12 Block 3: `LlmMayUsePredictor` Gemini-backed may-use predictor

**Files:**
- `crates/ferrite-ipi/Cargo.toml` — added `reqwest` (0.12, json + rustls-tls), `tokio` (full), `serde_json`
- `crates/ferrite-ipi/src/tool_decision/mod.rs` — added `LlmMayUsePredictor` struct with `from_env()` (reads `FERRITE_GEMINI_API_KEY`, returns `None` if absent) and `predict()` (async, temperature 0, 15s timeout, filters response against allowed tool list, returns empty set on any error)

Key design decisions:
- `from_env()` returns `Option<Self>` — callers must handle missing key gracefully (empty `may_use`)
- Reuses `RateLimiter::default_testing()` from `ferrite-agent` (2 req/s, burst 5)
- Response filtered against `available` allowlist — model cannot inject arbitrary tool IDs
- All network/parse errors silently return empty set (fail-safe)

### 2026-04-14 — Task 12 Block 2: `rule_based_must_use` keyword matcher

**Files:** `crates/ferrite-ipi/src/tool_decision/mod.rs`

- Added `rule_based_must_use(prompt: &str) -> HashSet<ToolId>`: case-insensitive keyword scan covering email, calendar, navigation, form, read/extract, download, JavaScript, and report/summarise intent clusters
- Returns empty set for unrecognised prompts (safe default for open-ended tasks)
- 4 new tests in `rule_tests` module: `email_prompt_gives_email_read`, `navigate_prompt_gives_navigate`, `open_ended_returns_empty`, `no_false_positives_on_unrelated_prompt`
- All 7 tests pass (4 new + 3 from Block 1)

### 2026-04-14 — Task 12 Block 1: `ferrite-ipi` crate skeleton + `ToolId` + `ToolFingerprint`

**Files:**
- `Cargo.toml` — added `crates/ferrite-ipi` to workspace `members`
- `crates/ferrite-ipi/Cargo.toml` — new crate; deps: `ferrite-agent`, `uuid` (v4), `serde` (derive), `thiserror`
- `crates/ferrite-ipi/src/lib.rs` — declares 7 public modules: `tool_decision`, `sanitizer`, `twin`, `containment`, `dry_run`, `comparator`, `dataset`
- `crates/ferrite-ipi/src/tool_decision/mod.rs` — implements `ToolId` (newtype over `String`, `From<&BrowserTool>` impl) and `ToolFingerprint` (`must_use`/`may_use` `HashSet<ToolId>`, `empty`, `is_empty`, `contains`, `merge`)
- `crates/ferrite-ipi/src/{sanitizer,twin,containment,dry_run,comparator,dataset}.rs` — empty stubs with comments

All 3 unit tests pass (`cargo test -p ferrite-ipi`):
- `tool_id_from_browser_tool_matches`
- `fingerprint_contains_checks_both_sets`
- `fingerprint_merge_accumulates`

### 2026-04-13 — ferrite-shell: double-click launches UI instead of smoke test

**Files:** `crates/ferrite-shell/src/main.rs`

- Changed default match arm in `main()` from `_ => run_smoke_test()` to `"smoke" => run_smoke_test()` + `_ => ferrite_ui::launch().expect(...)`.
- Running the executable with no arguments (e.g. double-clicking the `.exe`) now opens the Ferrite UI.
- Smoke test remains accessible via `ferrite-shell smoke` from the command line.

### 2026-04-13 — CI and Clippy fixes

**Files:**
- `.github/workflows/ci.yml` — removed `ubuntu-22.04` entry from the `matrix.include` list; matrix now covers only `windows-latest` and `macos-latest`
- `crates/ferrite-shell/src/main.rs` — fixed `print_literal` Clippy hard error: moved `"Errors"` from positional argument into the format string literal directly (`"{:<42} {:<12} {:<40} Errors"`)

### 2026-04-13 — Task 11 Block 2: agent sidebar panel in Iced UI

**Files:** `crates/ferrite-ui/src/lib.rs`

- Added 5 sidebar UI state fields (`show_agent_sidebar`, `agent_task_input`, `agent_tool_log`, `agent_response`, `agent_is_running`) + `agent_event_tx/rx` channel pair to `FerriteBrowser`
- Added 7 messages: `ToggleAgentSidebar`, `AgentTaskInputChanged`, `AgentTaskSubmitted`, `AgentToolLogged`, `AgentCompleted`, `AgentFailed`, `StopAgent`
- `AgentTaskSubmitted` handler: guards API key before spawning, creates `AgentTask`, clones `BrowserToolExecutor`, spawns tokio task that loops `run_turn` → sends `AgentToolLogged` per call → sends `AgentCompleted`/`AgentFailed`; stores `JoinHandle`
- `StopAgent`: aborts the handle, clears running flag
- Added `agent_event_sub` subscription (same `iced::stream::channel` + `Subscription::run_with_id` pattern as `tool_sub`, using `AgentEventChannel` marker type for dedup ID)
- Added "Agent" toggle button in toolbar (same active/inactive style as Audit/JS)
- Changed content layout to `row![browser_viewport, view_agent_sidebar(state)]` when sidebar is open
- Added `view_agent_sidebar()` function: 320px fixed-width sidebar with header (Stop button when running), task input (greyed container when running), Run Task button (disabled when running/empty), scrollable tool log ("Working..." animated dots), response box

**Exit condition:** `cargo build -p ferrite-ui` — zero errors.

### 2026-04-13 — Task 11 Block 1: tool executor bridge (agent ↔ Iced channel)

**Files:**
- `crates/ferrite-ui/Cargo.toml` — added `ferrite-agent`, `tokio`, `async-trait`
- `crates/ferrite-ui/src/lib.rs`:
  - Added `ToolRequest` struct (reply wrapped in `Arc<Mutex<Option<oneshot::Sender<...>>>>` for `Clone`/`Debug`)
  - Added `ToolRequestSender` / `ToolRequestReceiver` type aliases
  - Added `BrowserToolExecutor` implementing `ferrite_agent::ToolExecutor` via `oneshot` + mpsc channel
  - Added `tool_tx`, `tool_rx` (as `Arc<tokio::sync::Mutex<...>>` for `Fn` closure compatibility), `agent_handle` to `FerriteBrowser` state; unbounded channel initialized in `Default`
  - Added `FerriteBrowserMessage::ToolRequestArrived(ToolRequest)`
  - Added `ToolRequestArrived` arm in `update()` — dispatches to servo session methods; stubs "not yet implemented" for missing session APIs (`ReadPage`, `ExtractData`, `ClickElement`, `FillForm`)
  - Added `tool_sub` in `subscription()` using `iced::stream::channel` (iced 0.13 API) + `Subscription::run_with_id` for deduplication; merged into `Subscription::batch`

**API note:** iced 0.13 does not have `iced::subscription::channel` (spec wording); correct API is `iced::stream::channel(size, async_fn)` → stream, wrapped by `Subscription::run_with_id(id, stream)`.

**Exit condition met:** `cargo build -p ferrite-ui` — zero errors.

### 2026-04-13 — Task 10 Block 1: GeminiAgent with function calling

**Files:**
- `crates/ferrite-agent/Cargo.toml` — added `reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }`
- `crates/ferrite-agent/src/gemini.rs` — new file: `GeminiAgent` implementing `AgentRuntime`; `build_tool_manifest()` (9 function declarations); `parse_function_call()` mapping Gemini fn names → `BrowserTool`; `format_tool_results()` building `functionResponse` parts; full agentic loop (rate limit → POST → timeout → 429/error handling → fn-call dispatch → append to contents → repeat up to MAX_TURNS_PER_TASK)
- `crates/ferrite-agent/src/lib.rs` — added `pub mod gemini; pub use gemini::GeminiAgent;`
- `crates/ferrite-shell/Cargo.toml` — added `ferrite-agent`, `tokio`, `async-trait` deps
- `crates/ferrite-shell/src/main.rs` — added `agent-smoke` arm calling `GeminiAgent::from_env()` with `StubExecutor` (returns `"stub result for <tool_id>"`); skips gracefully when env var absent

**Exit condition met:** `cargo test -p ferrite-agent` — 4/4 pass. `cargo build -p ferrite-shell` — zero errors.

**Constants:** `GEMINI_API_BASE`, `DEFAULT_MODEL = "gemini-2.0-flash"`, `MAX_TURNS_PER_TASK = 10`, `TURN_TIMEOUT_SECS = 30`.

### 2026-04-13 — Task 9 Block 1: ferrite-agent crate

**Files:**
- `Cargo.toml` — added `crates/ferrite-agent` to workspace members
- `crates/ferrite-agent/Cargo.toml` — new crate with serde, serde_json, uuid (v4+serde), thiserror, async-trait, tokio deps
- `crates/ferrite-agent/src/lib.rs` — defined all public types: `BrowserTool` (9 variants with stable `tool_id()` strings), `AgentTask`, `AgentToolCall`, `AgentToolResult`, `AgentTurn`, `AgentError`, `AgentRuntime` trait, `ToolExecutor` trait, `RateLimiter` (token-bucket, 2 req/s default)

**Exit condition met:** `cargo test -p ferrite-agent` — all 4 unit tests pass.

**Note:** `uuid` required the `serde` feature (not just `v4`) for Serialize/Deserialize impls on `Uuid`.

### 2026-04-01 — Remove ferrite-capability-broker, ferrite-policy, ferrite-sandbox from active compilation

**Files:**
- `Cargo.toml` — three crates already removed from workspace members (were: ferrite-capability-broker, ferrite-policy, ferrite-sandbox)
- `crates/ferrite-shell/Cargo.toml` — removed the three dep lines
- `crates/ferrite-shell/src/main.rs` — removed broker/policy imports; removed "sandbox" match arm; added simplified `run_smoke_test()` that only exercises the audit log; removed top-level unused imports
- `crates/ferrite-servo/Cargo.toml` — removed ferrite-capability-broker dep
- `crates/ferrite-servo/src/shell.rs` — removed CapabilityBroker, Principal, PrincipalKind, BrokerDecision imports; removed broker/token/principal fields from FerriteWebViewDelegate, AppHandler, ServoShell; simplified `load_web_resource` to log-and-allow without broker check; removed `network_token_id()` and `broker()` public methods; removed `revoke_blocks_subsequent_requests` test; kept `audit_log_records_grants_and_denials` test
- `crates/ferrite-ui/Cargo.toml` — removed ferrite-capability-broker dep
- `crates/ferrite-ui/src/lib.rs` — removed ferrite_capability_broker import; removed `js_broker` field; simplified `JsExecuteRequested` handler to call `execute_js` directly without broker check

**Reason:** Extensions are not being built at this stage. The broker and policy engine exist solely to serve the extension sandbox. Removing them from active compilation keeps the workspace lean and CI green while extension work is deferred. Crates remain on disk untouched for future re-integration.

**Build:** `cargo build --workspace` — zero errors, zero warnings.

### 2026-03-30 — Replace all emoji/unicode icons with plain ASCII

**Files:** `crates/ferrite-ui/src/lib.rs`

- Iced's default font does not include emoji or many unicode symbols; they rendered as purple rectangles.
- Replaced every non-ASCII character in rendered text with ASCII equivalents:
  - Tab indicators: `>` (active), `-` (inactive), `...` (loading)
  - Nav buttons: `Back`, `Fwd`, `Reload`, `Stop`
  - Security badge: `HTTPS` (green) / `HTTP` (amber)
  - Logo: `Fe` (iron/ferrite chemical symbol)
  - Tile icons: `[D]`, `[R]`, `[G]`, `[S]`, `[N]`, `[W]`
  - Panel toggles: `+` / `v`
  - Error page: `ERR`
  - Close tab: `x`
  - Cmd symbol: `Cmd` (macOS) / `Ctrl` (other)
  - Ellipsis, em-dash: `...` / `-`
- Build clean. No non-ASCII in non-comment rendered text.

### 2026-03-30 — Fix home page, click, scroll

**Files:** `crates/ferrite-ui/src/lib.rs`

- **Home page fix**: The content branch order was wrong — Servo produces a blank white frame for `about:blank`, so the Servo-frame branch fired before the home page branch. Swapped order: `about:blank` check now runs first, home page always shown for that URL regardless of whether a Servo frame exists.
- **Click fix**: `on_press` / `on_release` in Iced 0.13 `mouse_area` don't carry a position — they fire a plain message. Changed `ServoMousePress { x, y }` / `ServoMouseRelease { x, y }` to `ServoMousePress` / `ServoMouseRelease` (no fields); the update handler reads `state.cursor_pos` (kept current by `on_move`) and uses that for the Servo input events.
- **Scroll fix**: Added `.on_scroll(|delta| ...)` to the `mouse_area` wrapping the Servo frame. `ScrollDelta::Lines` is converted to pixels (×60), `ScrollDelta::Pixels` passed through. `ServoScroll` message now carries only `delta_x`/`delta_y`; position comes from `state.cursor_pos` in the update handler.

### 2026-03-30 — Mouse/scroll interactivity, platform shortcuts, home page, smart URL

**Files:** `crates/ferrite-servo/src/session.rs`, `crates/ferrite-ui/src/lib.rs`

**ferrite-servo/src/session.rs — interaction API:**
- Added imports: `DevicePoint`, `DeviceVector2D`, `InputEvent`, `MouseButton`, `MouseButtonAction`, `MouseButtonEvent`, `MouseMoveEvent`, `Scroll`, `WebViewPoint`, `WheelDelta`, `WheelEvent`, `WheelMode`.
- Added `send_mouse_move(x, y)` — fires `InputEvent::MouseMove` to Servo WebView.
- Added `send_mouse_click(x, y)` — fires `MouseButtonAction::Down` + `Up` (Left button) for hit-testing.
- Added `send_mouse_down(x, y)` / `send_mouse_up(x, y)` — separate press/release for drag support.
- Added `send_right_click(x, y)` — Right mouse button down+up.
- Added `send_scroll(x, y, delta_x, delta_y)` — fires `InputEvent::Wheel` (pixel mode) and `notify_scroll_event(Scroll::Delta(...))` so Servo's compositor repaints.
- All methods have matching no-op stubs in the `#[cfg(not(feature = "servo"))]` block.

**crates/ferrite-ui/src/lib.rs — full rewrite:**
- Added `cursor_pos: (f32, f32)` and `content_y_offset: f32` to `FerriteBrowser` state.
- Added messages: `ServoMouseMove`, `ServoMousePress`, `ServoMouseRelease`, `ServoScroll`, `ContentAreaResized`.
- Servo frame now wrapped in `mouse_area` — `on_move` → `send_mouse_move`, `on_press` → `send_mouse_down`, `on_release` → `send_mouse_up` + `send_mouse_click`.
- **Smart URL resolver**: only adds `https://` when no scheme present and input looks like a hostname; bare words go to DuckDuckGo search.
- **Platform-aware shortcuts**: `#[cfg(target_os = "macos")]` selects `modifiers.command()` and `⌘` label; other OS uses `modifiers.control()` and `Ctrl` label.
- **New-tab/home page**: 6 quick-access emoji tiles (DuckDuckGo, Rust Docs, GitHub, Servo, Hacker News, Wikipedia) with hover shadows, large `⬡ ferrite` logo, keyboard shortcut reference panel at bottom.
- Tile row uses `.wrap()` so it reflows on narrow windows.
- JS console, Audit panel, toolbar all updated to new palette (`C_ACCENT_BRIGHT`, `C_DANGER`).

### 2026-03-30 — Build fixes + UI overhaul + JS console

**Files:** `crates/ferrite-servo/src/session.rs`, `crates/ferrite-ui/src/lib.rs`

**servo/src/session.rs — 3 build fixes:**
- Renamed `notify_console_message` → `show_console_message` (correct servo v0.0.5 `WebViewDelegate` method name).
- Changed parameter type from `servo::ConsoleSender` → `(level: servo::ConsoleLogLevel, message: String)`.
- `evaluate_javascript` callback now correctly receives `Result<JSValue, JavaScriptEvaluationError>` (not `Option<_>`); result serialised via `format!("{:?}", v)`.
- `ConsoleLogLevel::Error` comparison uses `matches!` macro since `ConsoleLogLevel` has no `PartialEq`.
- `cargo build -p ferrite-servo --features servo` now passes clean.

**ferrite-ui/src/lib.rs — full overhaul:**
- Restored hand-crafted dark palette constants (`C_BASE`, `C_SURFACE`, `C_ACCENT`, …) plus added `C_ACCENT_BRIGHT`, `C_DANGER`.
- Added JS console state: `show_js_console`, `js_input`, `js_output`, `js_broker` with a minted `JsExecute` token.
- Added messages: `ToggleJsConsole`, `JsInputChanged`, `JsExecuteRequested`, `JsConsoleClear`.
- Added `JsExecuteRequested` handler: broker.check → if Granted calls `session.execute_js()`; if Denied appends BLOCKED message.
- Fixed "stuck on Loading": Servo frame branch now checked BEFORE the `about:blank` new-tab branch, so first frame renders immediately when available.
- Refactored toolbar: nav buttons + address bar + Audit and JS toggle buttons all in one row (no separate audit-toolbar row).
- JS console panel: 260 px drawer with header/output/input row, `Run ↵` button, Ctrl+J shortcut, Enter-to-submit.
- Panels are mutually exclusive (opening one closes the other).
- Better icons: `←` `→` `↺` `✕` `🔒` `⚠` `◉`/`○`/`⊙` tab indicators, `⬡` logo, emoji shortcuts on new-tab page.
- New keyboard shortcuts: F12 = toggle Audit, Ctrl+J = toggle JS console.
- `column(layout)` wrapped in `container` (Iced 0.13: column has no `.style()` method).
- All warnings resolved: removed `#[allow(dead_code)]` by using the constants.

### 2026-03-30 — UI visual redesign: theme-derived palette + updated layout constants

**Files:** `crates/ferrite-ui/src/lib.rs`

- Replaced all hardcoded `C_*` colour constants with a `Palette` struct and `palette()` helper that derives colours from `Theme::Dark`'s `extended_palette()` at runtime: background base/weak/strong for layers, primary strong for accent, danger base for error states.
- Updated layout constants: `TOOLBAR_HEIGHT` 44, `TAB_BAR_HEIGHT` 36, `BORDER_RADIUS` 6, `PANEL_PADDING` 8; removed `#[allow(dead_code)]` from `TAB_MIN_WIDTH`/`TAB_MAX_WIDTH`.
- Toolbar nav buttons now render as explicit 32×32 containers with icon text size 16; `spacing(4)` throughout toolbar.
- Address bar pill radius updated to 16px (true pill), height fixed at 32px.
- Lock icon uses 🔒 emoji: green (`p_safe`) for https, grey (`p_text_dim`) for http/other, empty for about:blank.
- Progress bar height increased to 3px.
- Audit panel drawer gains rounded top corners and a subtle upward shadow via `iced::Shadow`.
- All palette-derived style closures capture colour values as `Copy` locals so closures remain `'static`.
- `cargo check -p ferrite-ui` passes with zero errors and zero warnings.

### 2026-03-26 — Fix segfault with 2+ tabs + full UI visual overhaul

**Files:** `crates/ferrite-servo/src/session.rs`, `crates/ferrite-ui/src/lib.rs`

**Segfault fix:**
- Root cause: `spin()` (which calls `servo.spin_event_loop()`) was being called once per tab per tick — N calls per tick for N tabs. Since all tabs share one `Servo`, this double-processes paint messages and triggers a segfault inside Servo's compositor.
- Fix: split `spin()` into `pump_engine()` (call once per tick, on any session) and `sync_and_read()` (call on every session). The `ServoFrame` handler now calls `pump_engine` once then `sync_and_read` on all sessions.

**UI overhaul:**
- Replaced `Theme::Dark` palette references with a hand-crafted dark colour system: `C_BASE`, `C_SURFACE`, `C_RAISED`, `C_DIVIDER`, `C_TEXT`, `C_TEXT_DIM`, `C_ACCENT` (electric indigo), `C_INPUT`, `C_SAFE`, `C_WARN`
- Tab bar: tabs use rounded top corners; active tab drops to `C_BASE` (feels attached to content); accent underline uses `C_ACCENT`; inactive tabs show `C_TEXT_DIM` labels, brightening on hover
- Navigation toolbar: single-character `‹` `›` `↺` glyphs sized for crisp rendering; pill address bar with `C_INPUT` background and `C_ACCENT` focus ring; `⚿` HTTPS indicator in `C_SAFE` green
- Progress bar: 2 px pulsing `C_ACCENT` stripe reserved at all times to prevent content layout shift
- New-tab page: `⬡` hexagon logo mark at 52 px; wordmark at 36 px; shortcut cards with `C_SURFACE` background and subtle border, hex icons; large search bar with 26 px border radius
- Error page: accent-coloured primary CTA button with shadow; cleaner copy
- Audit panel: `C_RAISED` header row, `C_TEXT_DIM` column labels, tighter row spacing
- All style functions now use palette constants directly instead of `theme.extended_palette()` tokens
- `cargo build --features ferrite-servo/servo -p ferrite-shell` — ✅

### 2026-03-26 — Fix tab isolation: wrong page shown after switching tabs

**File:** `crates/ferrite-servo/src/session.rs`

- Root cause: GL context is a per-thread global. `spin_event_loop()` drives all WebViews and calls `make_current()` on each painter's rendering context as it renders. After the loop, the last context to render (tab 2) is left as "current". When tab 1's `spin()` then calls `read_to_image` (which uses `glReadPixels`), it reads from whichever surface was last made current — tab 2's — giving the wrong pixels.
- Fix: call `self.rendering_context.make_current()` immediately before `read_to_image` in `spin()` to re-establish the correct GL context for this tab before the readback.
- `cargo build --features ferrite-servo/servo -p ferrite-shell` — ✅ passes

### 2026-03-26 — Fix "Already initialized" panic when opening multiple tabs

**File:** `crates/ferrite-servo/src/session.rs`

- Root cause: `ServoBuilder::default().build()` calls `opts::initialize_options()` which uses a process-wide global and panics if called more than once. Every `HeadlessServoSession::new()` call (one per tab) was triggering this.
- Fix: added a `thread_local! { static SERVO_ENGINE: RefCell<Option<Servo>> }` singleton and a `get_or_init_servo()` helper that builds the `Servo` engine on the first call and `.clone()`s the `Rc` wrapper on all subsequent calls. `ServoBuilder` is only invoked once per process.
- `cargo build --features ferrite-servo/servo -p ferrite-shell` — ✅ passes

### 2026-03-25 — Fix go_back/go_forward/stop API mismatches in ferrite-servo

**File:** `crates/ferrite-servo/src/session.rs`

- `go_back()`: passed `1` as the required `amount: usize` argument (servo `WebView::go_back` signature changed to take a step count)
- `go_forward()`: same fix — passed `1` as the `amount: usize` argument
- `stop()`: `WebView::stop()` does not exist in this servo build; replaced with a `tracing::warn!` no-op and a TODO comment until servo exposes the method
- `cargo check -p ferrite-servo` — ✅ passes

### 2026-03-25 — Visual redesign of Iced UI shell

**File:** `crates/ferrite-ui/src/lib.rs`

- Added layout constants: `TOOLBAR_HEIGHT` (44), `TAB_BAR_HEIGHT` (36), `TAB_MIN_WIDTH` (120), `TAB_MAX_WIDTH` (240), `BORDER_RADIUS` (6), `PANEL_PADDING` (8)
- Added `Border` to iced imports for cleaner style declarations
- **Tab bar** — now uses `scrollable::Direction::Horizontal` so the strip scrolls when tabs overflow; each tab is a `column![label+close row, underline strip]` so the 2 px accent-colour bottom border on the active tab is rendered without touching `button::Style`; close (×) button has `text_color.a = 0.0` by default and becomes visible on hover; + button uses `padding([10, 10])` for a ~36 × 36 appearance; labels truncated to 25 chars
- **Separators** — 1 px `container` using `palette.background.strong.color` added between tab bar and nav toolbar, and between nav toolbar and viewport
- **Navigation toolbar** — fixed height `TOOLBAR_HEIGHT`; back/forward/reload/stop buttons use `padding([8, 8])` + `text.size(16)` for ~32 × 32 rounded squares; new `nav_btn_style` handles `button::Status::Disabled` (alpha 0.25)
- **Address bar** — pill shape via `text_input::Style` with `border.radius = 16.0`; focused state draws a 1.5 px primary-colour border; background = `palette.background.strong.color`; lock icon is 🔒 (green #33CC66) for HTTPS and 🔓 (muted grey) for HTTP/other
- **Colour palette** updated throughout: toolbar background = `palette.background.base.color` (darkest); tab bar = `palette.background.weak.color`; active tab = `palette.primary.base.color` at 15 % alpha; inactive tab text at 70 % alpha
- **Audit panel** — rounded top corners (`top_left: BORDER_RADIUS, top_right: BORDER_RADIUS`) via `iced::border::Radius` struct; 1 px `palette.background.strong.color` border for shadow effect
- All button text sizes set to 14; toolbar element spacing 4 px; `PANEL_PADDING` (8) used consistently for horizontal padding in all toolbar rows and audit panel cells
- Fixed `scrollable::Scrollbar::new()` call (iced 0.13 takes no arguments)
- `cargo check` ✅ · `cargo fmt` ✅

### 2026-03-25 — Navigation controls, load status tracking, and progress bar

**Files:** `crates/ferrite-servo/src/session.rs`, `crates/ferrite-ui/src/lib.rs`

#### `ferrite-servo` — `session.rs`

- Added `pub enum LoadStatus { Loading, Complete, Failed(String) }` at module level (always compiled, no feature gate) so `ferrite-ui` can import it without the `servo` feature
- Added three shared `Rc<RefCell<>>` cells to `HeadlessDelegate`: `load_status`, `current_url`, `nav_count`
- Implemented `notify_load_status_changed` in `HeadlessDelegate`: on `servo::LoadStatus::Complete` updates shared URL and increments nav count; all other statuses set `LoadStatus::Loading`
- Added five new fields to `HeadlessServoSession` (servo feature): `last_load_status`, `current_url`, `shared_load_status`, `shared_url`, `shared_nav_count`
- Updated `spin()` to sync `last_load_status` and `current_url` from the shared cells after each `spin_event_loop()` call
- Added new public methods to `HeadlessServoSession`:
  - `load_status() -> &LoadStatus` — exposes synced load state
  - `current_url() -> &str` — exposes synced current URL
  - `can_go_back() -> bool` — true when nav_count > 1
  - `can_go_forward() -> bool` — false (forward history not yet tracked)
  - `go_back()`, `go_forward()`, `reload()`, `stop()` — call corresponding servo `WebView` methods
- Added stub implementations of all new methods to the non-servo (`#[cfg(not(feature = "servo"))]`) build

#### `ferrite-ui` — `lib.rs`

- Imported `ferrite_servo::session::LoadStatus`
- Added state fields: `is_loading: bool`, `can_go_back: bool`, `can_go_forward: bool`, `progress_offset: f32`
- Added messages: `GoBack`, `GoForward`, `Reload`, `StopLoading`, `LoadStatusChanged { tab, status, url }`
- Updated `update()` handlers:
  - `NavigateRequested` — sets `is_loading = true` immediately on submit
  - `GoBack/GoForward/Reload` — calls matching session method, sets `is_loading = true`
  - `StopLoading` — calls `session.stop()`, sets `is_loading = false`
  - `LoadStatusChanged` — updates `is_loading`, syncs `address_bar_input` and `tab_urls` when URL differs from typed value (handles server-side redirects)
  - `SelectTab` / `CloseTab` — sync `is_loading`/`can_go_back`/`can_go_forward` from new active session
  - `ServoFrame` — advances `progress_offset`; after spinning all sessions, reads load state from active tab and emits `LoadStatusChanged` if status or URL changed; updates `can_go_back`/`can_go_forward` directly
- Replaced the old address bar section in `view()` with a navigation toolbar:
  - `[←]` (disabled when `can_go_back = false`) → `GoBack`
  - `[→]` (disabled when `can_go_forward = false`) → `GoForward`
  - `[⟳]` when idle → `Reload`; `[✕]` when loading → `StopLoading`
  - `🔒` lock icon prefix shown when committed URL starts with `https://`
  - Address bar fills remaining width
- Added 3 px indeterminate progress bar below nav toolbar (visible only when `is_loading = true`); alpha pulses via `sin(progress_offset × 2π)` using theme primary colour

#### Build status

- `cargo check -p ferrite-servo -p ferrite-ui` — ✅ passes (no feature flags needed)
- `cargo fmt -p ferrite-servo -p ferrite-ui` — ✅ clean

### 2026-03-25 — Error page, new-tab page, tab titles, favicon placeholders

**Files:** `crates/ferrite-servo/src/session.rs`, `crates/ferrite-ui/src/lib.rs`

#### `ferrite-servo` — `session.rs`
- Added `page_title: Rc<RefCell<Option<String>>>` shared cell to `HeadlessDelegate`
- Implemented `notify_page_title_changed` in `HeadlessDelegate`: stores received title in shared cell
- Added `shared_page_title` and `last_page_title` fields to `HeadlessServoSession`
- `spin()` now syncs `last_page_title` from the shared cell each tick
- Added `pub fn page_title(&self) -> Option<&str>` — returns last synced title, or `None`
- Added stub `page_title()` returning `None` in non-servo build

#### `ferrite-ui` — `lib.rs`
- Added state fields: `tab_error: Vec<Option<String>>`, `tab_titles: Vec<String>`, `new_tab_search_input: String`
- Added message `NewTabSearchChanged(String)` for the new-tab search field
- `AddTab` initialises `tab_error` and `tab_titles` entries for each new tab; sets `address_bar_input` to empty string (was `about:blank`)
- `CloseTab` removes corresponding entries from `tab_error` and `tab_titles`
- `NavigateRequested` clears `tab_error[active_tab]` and `new_tab_search_input` on each navigation
- `LoadStatusChanged` with `status="failed"` stores the URL as the error message in `tab_error[tab]`; non-failed statuses clear `tab_error[tab]` as before
- `ServoFrame` syncs `tab_titles[active]` from `session.page_title()` each tick if title is non-empty
- **Content area** now has a 4-level priority chain:
  1. **Error page** — when `tab_error[active]` is `Some`: centred column with ⚠ icon (red, size 48), "Could not load page" heading, failed URL (muted), error message (muted), "Try Again" → `Reload`, "Go Home" → `NavigateRequested("https://lite.duckduckgo.com")`
  2. **New-tab page** — when `tab_urls[active] == "about:blank"`: centred column with "ferrite" logo (size 48), subtitle, 480 px pill-shaped search input bound to `new_tab_search_input` (submit → `NavigateRequested` with `resolve_url`), quick-access buttons for DuckDuckGo / Rust Docs / Servo
  3. **Servo frame** — live rendered pixels from the active session
  4. **Loading placeholder** — "Loading…" text while session initialises
- **Tab bar** now uses `tab_titles` instead of `tabs` for display labels (truncated to 22 chars)
- Each tab label is prefixed with a favicon placeholder: ⟳ when that tab is loading, 🌐 otherwise

#### Build status
- `cargo check -p ferrite-servo -p ferrite-ui` — ✅ passes
- `cargo fmt -p ferrite-servo -p ferrite-ui` — ✅ clean

### 2026-03-25 — Keyboard shortcuts, smart URL resolution, address bar focus

**Files:** `crates/ferrite-ui/src/lib.rs`, `crates/ferrite-ui/Cargo.toml`

#### `ferrite-ui/Cargo.toml`
- Added `urlencoding = "2"` dependency for query-string encoding in DuckDuckGo search fallback

#### `ferrite-ui` — `lib.rs`
- Added `use iced::keyboard` to imports
- Added `const ADDRESS_BAR_ID: &str = "ferrite_address_bar"` for text_input focus targeting
- Added `address_bar_focused: bool` to `FerriteBrowser` state (default `false`)
- Added messages: `FocusAddressBar`, `ClearAddressBarFocus`, `CloseActiveTab`, `EscapePressed`
  - `CloseActiveTab` dispatches `CloseTab(active_tab)` to avoid capturing state in `on_key_press` fn pointer
  - `EscapePressed` dispatches `StopLoading` if loading, otherwise clears address bar focus
- Updated `NavigateRequested` handler: calls `resolve_url()` before navigation; updates `address_bar_input` with the resolved URL (so typed `google.com` displays as `https://google.com`); sets `address_bar_focused = false`
- Added `FocusAddressBar` handler: sets `address_bar_focused = true`, returns `text_input::focus(Id::new(ADDRESS_BAR_ID))` task
- Added `ClearAddressBarFocus` handler: sets `address_bar_focused = false`
- Added `fn resolve_url(input: &str) -> String`:
  1. Already has `http://` or `https://` → use as-is
  2. No spaces and contains `.` → prepend `https://`
  3. Otherwise → `https://lite.duckduckgo.com/lite/?q=<urlencoded>`
- Added `.id(text_input::Id::new(ADDRESS_BAR_ID))` to address bar widget in `view()`
- Updated `subscription()` to use `Subscription::batch([keyboard_sub, servo_tick])`
  - Keyboard handler extracted as free fn `handle_key_press(key, modifiers)` (function pointer, no captures)
  - Shortcuts: Ctrl+T → AddTab, Ctrl+W → CloseActiveTab, Ctrl+R → Reload, Ctrl+L → FocusAddressBar, F5 → Reload, Alt+Left → GoBack, Alt+Right → GoForward, Escape → EscapePressed

#### Build status
- `cargo check -p ferrite-ui` — ✅ passes
- `cargo fmt -p ferrite-ui` — ✅ clean

### 2026-03-25 — CI workflow updated

**File:** `.github/workflows/ci.yml`

- Added `.devcontainer/**` to path triggers so CI runs when devcontainer config changes
- Pinned runner to `ubuntu-22.04` (was `ubuntu-latest`) to match the devcontainer base image
- Added "Install system dependencies" step with all Servo/Iced native deps (clang, lld, gstreamer stack, libxcb, libssl, sqlite3, etc.)
- Added `wasm32-unknown-unknown` target to the Rust toolchain install step
- Added explicit `cargo fetch` step before lint/test
- Added "Build hello-ext Wasm" step: builds `extensions/hello-ext` targeting `wasm32-unknown-unknown --release`

> **Note (superseded 2026-04-13):** the Linux/ubuntu CI job and the hello-ext Wasm build step were later removed; CI now runs Windows + macOS only. Retained here for history.

### 2026-03-25 — Devcontainer configuration completed

**Files:** `.devcontainer/devcontainer.json`, `.devcontainer/.dockerignore` (at repo root `Major Project/`)

- `devcontainer.json`: configures the dev container with name "Ferrite Browser Dev", build context at repo root, workspace mounted at `/workspace`, two named volumes for Cargo registry and build target caches, port 9222 forwarded (Ferrite Agent WebSocket), rust-analyzer/crates/even-better-toml/vscode-lldb extensions, clippy-on-save with `-D warnings`, `postCreateCommand` runs `cargo fetch` on container creation
- `.dockerignore`: excludes `target/`, `.git/`, `*.pdf`, `*.pptx`, `*.html` from Docker build context to keep image builds fast

### 2026-03-25 — Devcontainer Dockerfile created

**File:** `.devcontainer/Dockerfile` (at repo root `Major Project/`)

Created the devcontainer Dockerfile for Linux-based development (Ubuntu 22.04):
- Installs all Servo build dependencies: clang, lld, cmake, gstreamer stack, libxcb, libssl, libdbus, libfreetype, libfontconfig, sqlite3, etc.
- Installs Rust stable via rustup with `wasm32-unknown-unknown` target, clippy, rustfmt, rust-analyzer
- Pre-warms Cargo registry by copying workspace manifests + stub sources and running `cargo fetch` — this layer is cached unless dependencies change
- Sets `WORKDIR /workspace` for actual development use

### 2026-03-25 — Task 1 Block 4: WebView creation wired in (navigate to real URL)

**File:** `crates/ferrite-servo/src/shell.rs`

Activated the `WindowRenderingContext` + WebView construction that was previously commented out pending the surfman/GL setup:
- `servo::WindowRenderingContext::new(display, whandle, size)` — creates the GL surface from the winit window handles
- `WebViewBuilder::new(&servo, rc).delegate(...).url("https://example.com").build()` — creates the WebView with `FerriteWebViewDelegate` wired in
- `webview.resize(window.inner_size())` — sizes the render surface to fill the window
- `self.webview = Some(webview)` — the `Option<servo::WebView>` field is now populated

`cargo build -p ferrite-servo` (without `--features servo`) still compiles cleanly in 17s — the new code is gated behind `#[cfg(feature = "servo")]`.

To test the full rendering path:
```
cargo run -p ferrite-shell --features ferrite-servo/servo window
```
First build takes ~10-20 min (compiles Servo from source).

### 2026-03-24 — Workspace scaffolded + Broker + Audit Log implemented

> **Historical note:** This entry and several below reference the capability broker, policy
> engine, and Extism sandbox, which were removed from the active workspace on 2026-04-01
> (see that entry). They remain on disk for possible future re-integration. The audit-log
> work described here remains current. Entries are preserved verbatim for forensic history.

**Environment**
- IDE: Google Antigravity (installed, VS Code fork)
- Terminal: PowerShell (Windows, no WSL2)
- Rust toolchain: stable MSVC
- Build tools: Visual Studio C++ Build Tools installed

**Workspace**
- Initialized Cargo workspace at `Browser/` with `resolver = "2"`
- Crates created under `Browser/crates/` (broker/policy since deferred)
- `cargo build` passes cleanly

### `ferrite-audit-log` — COMPLETE

**File:** `crates/ferrite-audit-log/src/lib.rs`

**Dependencies (current):**
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
rusqlite = { version = "0.37", features = ["bundled"] }
thiserror = "1"
```

**What was implemented:**
- `AuditEventKind` enum — `CapabilityGranted`, `CapabilityDenied`, `CapabilityExercised`, `ContentBlocked`
- `AuditEntry` struct — full entry with `entry_id`, `sequence`, `timestamp`, `kind`, `principal_id`, `capability`, `url`, `prev_hash`, `entry_hash`
- `AuditError` enum (thiserror) — `HashMismatch`, `ChainBroken`, `Sql`, `Parse`, `Serialization`, `ChainBrokenLoad`
- `AuditLog` struct — in-memory log with `Vec<AuditEntry>` and `sequence_counter`
- `AuditLog::append()` — computes SHA-256 over `"{sequence}{timestamp}{kind:?}{principal_id}{prev_hash}"`, stores entry
- `AuditLog::verify_chain()` — recomputes every hash and validates prev_hash linkage, returns `bool`
- `PersistentAuditLog` struct — wraps `AuditLog` + `rusqlite::Connection`
- `PersistentAuditLog::new()` — opens SQLite, creates `audit_entries` table if not exists
- `PersistentAuditLog::append()` — appends to in-memory log AND inserts row into SQLite atomically
- `PersistentAuditLog::load()` — reads all rows ordered by sequence, reconstructs log, calls `verify_chain()`, errors if chain broken

### `ferrite-servo` — SCAFFOLDED (2026-03-24)

**Files created:**
- `crates/ferrite-servo/Cargo.toml` — dependencies: `servo` (git tag v0.0.5), `raw-window-handle 0.6`, `winit 0.30`, `log 0.4`
- `crates/ferrite-servo/src/lib.rs` — declares `pub mod shell`
- `crates/ferrite-servo/src/shell.rs` — empty stub, awaiting Block 2 implementation

**Root `Cargo.toml`** — added `"crates/ferrite-servo"` to workspace members.

**Verified:** `cargo metadata --no-deps` lists all workspace members correctly.

### `ServoShell` — winit shell + Servo integration skeleton (2026-03-24)

**File:** `crates/ferrite-servo/src/shell.rs`

**API research finding:** The Servo v0.0.5 embedding API does **not** use a `WindowMethods` trait or methods like `get_coordinates()` / `get_gl_context()` — those are from an older prototype. The actual API (researched from upstream repo) is:
- `ServoBuilder::default()` — builder pattern, not `Servo::new()`
- `servo.set_delegate(ServoDelegate)` — global callbacks
- `WebView` — per-tab handle; created via Servo, accessed via `WebViewDelegate` callbacks
- `webview.load(ServoUrl)`, `webview.resize(Size2D)`, `webview.paint()`
- `servo.spin_event_loop()` — drives Servo's internal task queue

**What was implemented:**
- `FerriteWebViewDelegate` — implements `servo::WebViewDelegate`; `notify_new_frame_ready` calls `webview.paint()`
- `FerriteServoDelegate` — implements `servo::ServoDelegate`; all methods default no-ops for now
- `AppHandler` struct (winit 0.30 `ApplicationHandler`):
  - `resumed()` — creates winit window 1280×800; `#[cfg(feature = "servo")]`: builds Servo via `ServoBuilder`, sets delegate, creates WebView, loads `about:blank`, calls `spin_event_loop()` once
  - `window_event()` — `CloseRequested` → exit; `RedrawRequested` → `spin_event_loop()` + `webview.paint()` (servo feature) + `request_redraw()`
- `ServoShell` — `new()` + `run(self)`, `Default` impl

**`ferrite-shell/Cargo.toml`** — added `ferrite-servo = { path = "../ferrite-servo" }`

**`ferrite-shell/src/main.rs`** — smoke test moved to `fn run_smoke_test()`:
- First CLI arg `"window"` → `ServoShell::new().run()` (opens window)
- Anything else → `run_smoke_test()`

**Feature flag:** `ferrite-servo/Cargo.toml` has `[features] servo = []`. Servo code activates with `--features servo`. Build without the feature gives the bare winit shell.

### `ferrite-ui` — Iced UI shell (2026-03-25)

**Files created:**
- `crates/ferrite-ui/Cargo.toml` — `iced = { version = "0.13", features = ["tokio"] }`
- `crates/ferrite-ui/src/lib.rs`

**Workspace changes:**
- `Browser/Cargo.toml` — added `"crates/ferrite-ui"` to workspace members
- `crates/ferrite-shell/Cargo.toml` — added `ferrite-ui = { path = "../ferrite-ui" }`
- `crates/ferrite-shell/src/main.rs` — added `"ui"` arm to CLI match → calls `ferrite_ui::launch()`

**Iced 0.13 API note:**
Iced 0.13 replaced the `Application` trait with a functional builder pattern.
`iced::application(title, update, view)` returns an `Application` builder; `.theme()`, `.window_size()`, `.centered()`, `.run()` are chained on it.

**What was implemented (initial):**
- `FerriteBrowser` — state struct
- `FerriteBrowserMessage` — message enum
- `update(state, message) -> Task<FerriteBrowserMessage>`
- `view(state) -> Element<'_, FerriteBrowserMessage>`
- `pub fn launch() -> iced::Result` — `iced::application("Ferrite Browser", update, view).window_size(1280×800).centered().theme(|_| Theme::Dark).run()`

### `ferrite-ui` — tab bar + address bar (2026-03-25)

**File:** `crates/ferrite-ui/src/lib.rs`

- Tab bar: `tabs: Vec<String>` + `active_tab: usize`; `AddTab` / `CloseTab(usize)` / `SelectTab(usize)`; close button disabled when only one tab remains
- Address bar: `address_bar_input: String` + `tab_urls: Vec<String>` (parallel to `tabs`); `AddressBarChanged` / `NavigateRequested`; tab switches reflect the active tab's URL back into the address bar

### `ferrite-servo` + `ferrite-ui` — Task 5 Block 1: Servo embedded in Iced (2026-03-25)

**Files created/changed:**
- `crates/ferrite-servo/src/session.rs` — new `HeadlessServoSession` type
- `crates/ferrite-servo/src/lib.rs` — added `pub mod session`
- `crates/ferrite-ui/Cargo.toml` — added `ferrite-servo`, `iced_widget` (with `image` feature)
- `crates/ferrite-ui/src/lib.rs` — servo state, messages, subscription, frame display

**`HeadlessServoSession` (ferrite-servo/src/session.rs):**
- Uses `SoftwareRenderingContext` (CPU rasteriser — no GPU/window handle required, no winit event loop)
- The stub type (`#[cfg(not(feature = "servo"))]`) compiles without the feature and returns `Err` from `new()` so the UI degrades gracefully
- `new(width, height)` — creates rendering context, audit log at `$TMPDIR/ferrite_servo_session.db`, Servo engine, WebView loaded to `about:blank`
- `navigate(&str)` — calls `webview.load(parsed_url)`
- `spin()` — calls `servo.spin_event_loop()` then `read_to_image(DeviceIntRect)` to capture RGBA frame
- `get_frame() -> Option<(u32, u32, Vec<u8>)>` — returns latest frame pixels
- `resize(w, h)` — resizes rendering context and WebView

**Iced integration (ferrite-ui/src/lib.rs):**
- `FerriteBrowser` gains `servo_shell: Option<HeadlessServoSession>` and `servo_frame: Option<(u32, u32, Vec<u8>)>`
- `subscription(state)` — returns `time::every(16ms).map(|_| ServoFrame)` when session is active
- `update()` `ServoFrame` arm — calls `session.spin()`, stores `get_frame()` result
- `view()` content area — renders `ServoImage::new(ImageHandle::from_rgba(w, h, bytes))` when a frame exists

### `ferrite-ui` — Task 5 Block 2: per-tab Servo sessions (2026-03-25)

**File changed:** `crates/ferrite-ui/src/lib.rs`

- `servo_shell` replaced with `servo_sessions: HashMap<usize, HeadlessServoSession>` keyed by tab index
- `AddTab` creates a new session; `CloseTab(i)` removes and re-keys; `ServoFrame` tick spins all sessions
- `view()` reads from `servo_sessions[active_tab]`

### `ferrite-servo/session.rs` — rustls CryptoProvider panic fixed (2026-03-25)

- **Problem:** `thread 'ResourceManager' panicked: Could not automatically determine the process-level CryptoProvider` — rustls 0.23 requires `CryptoProvider::install_default()` once before any TLS work
- **Fix:** Added `aws_lc_rs::default_provider().install_default()` at the top of `HeadlessServoSession::new()`; returns `Err` silently if already installed
- **Verified:** `cargo build -p ferrite-shell --features ferrite-servo/servo` — clean build, 15s

### `ferrite-servo/session.rs` — compile errors fixed (2026-03-25)

- `surfman::error::Error` doesn't implement `Display` — changed `{}` → `{:?}` in error format strings
- Removed unused `use url::Url` import
- Forced `aws_lc_rs` rustls backend via explicit dep with feature
- **Verified:** `cargo check -p ferrite-servo --features servo` — zero errors

---

## What To Do Next (pick up here)

> Design is complete and frozen: `EVALUATION_PLAN.md` (evaluation-facing), `CLAUDE.md` →
> *Tool Vocabulary and Capability Model* (vocabulary canon), and `FINALIZED_DECISIONS.md`
> (rationale, Decisions 1–6) are the authoritative references. No design decisions remain.
> The work below is implementation, in dependency order.

1. **Execute the pre-Task-19 vocab-fix block** (upstream of the dataset; touches `tool_decision`,
   `dry_run.rs`, `comparator.rs` only — no Servo). Per `CLAUDE.md` and `FINALIZED_DECISIONS.md`:
   - rewrite `rule_based_must_use` + the predictor allowlist to emit ONLY the capability
     vocabulary (six action classes; cut all phantom tool IDs — `email.*`, `calendar.*`,
     `form.submit`, `report.write`, `contacts.*`, `storage.*`, `network.fetch`, `screenshot`);
   - replace the lossy `DryRunRecord.tools_called: HashSet<ToolId>` with an ordered, origin-bound
     event log (`Vec<{ tool, origin }>`); track `current_origin` in `RecordingExecutor`
     (update on `Navigate`, seed from `AgentTask.context_url`);
   - rewrite `compare()` as lower-then-compare with per-origin attribution + specificity
     precedence (exact > domain_suffix > task_open) + the general `unscopable` rule
     (`js.execute` always a deviation);
   - unify key loading: ONE shared loader (env `FERRITE_GEMINI_API_KEY` first, then
     `gemini_key.txt` next to the exe) used by BOTH `gemini.rs` and `tool_decision`; warn (not
     fail) if the predictor initializes keyless during an eval run.
   - The existing `comparator.rs` tests encode the old single-vocabulary model and will be
     rewritten as part of this block (expected, not a regression).

2. **Task 18 — defense-mode toggle.** `DefenseMode { On, SanitizerOnly, Off }`; switchable via
   setter + `FERRITE_DEFENSE` env var. `Off` bypasses the whole predict→dry-run→compare→consent
   loop; `SanitizerOnly` runs the sanitizer but bypasses the loop. For the §4 baseline + ablation.

3. **Task 19 — dataset pipeline (`dataset.rs`).** Implement to the finalized two-layer schema in
   `EVALUATION_PLAN.md` §7 (CaseDefinition + ExecutionRecord; the `GroundTruth` enum). Flat
   `src/dataset.rs` (not `src/dataset/mod.rs`); `rusqlite` `0.37` bundled; test temp paths via
   `std::env::temp_dir()`. Supersedes the thin `IpiEvent`/`IpiLabel` in `comparator.rs`.

4. **Task 20 — sanitizer T1b extension**, then **Tasks 21–22** (`ferrite-eval` harness +
   AgentDojo Slack adapter). Measurement-gated values (corpus N, AgentDojo Slack count,
   category-5 Tier A/B) are settled by a pilot/spike during corpus construction, not now.

5. **Re-verify the full workspace build/test matrix** (recorded from prior sessions, not
   necessarily re-run today):
   ```
   cargo build --workspace
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   ```

6. **Servo feature build check** — `cargo build -p ferrite-shell --features ferrite-servo/servo`
   should compile end-to-end; `cargo run -p ferrite-shell ui` to confirm the viewport renders
   Servo frames. First build compiles Servo from source (~10–20 min).

---

## Known Issues / Notes

- `check.txt`, `check2.txt`, `check_output.txt` exist in `Browser/` root — scratch files from earlier testing. Consider deleting.
- **CI does not cover Linux.** The matrix is Windows + macOS only (Linux removed 2026-04-13). The Linux-only network-namespace path in `ferrite-ipi::containment` (component 4, via `nix`) is therefore never compiled by CI. Account for this when reasoning about test coverage, and note it as a limitation in any evaluation writeup.
- Mixed editions across the workspace: `ferrite-audit-log` is `edition = "2024"`, `ferrite-ipi` is `edition = "2021"`. Not a bug, but standardise deliberately if desired rather than letting it drift.
- Servo dependency: package is `libservo` (git, tag `v0.0.5`), lib name `servo` (so imports use `use servo::...`), specified as `optional = true` and enabled via `--features servo`.
- `rusqlite` is `0.37` with `features = ["bundled"]` workspace-wide. It was bumped from an earlier `0.31` to resolve a `libsqlite3-sys` link conflict with `libservo` (which requires `rusqlite ^0.37`). Never reintroduce a second rusqlite version.
- Inherited Dependabot alerts transitive from Servo v0.0.5 are non-actionable until the next Servo bump.
- **Key-loading divergence (to fix in the vocab-fix block).** `gemini.rs` reads the Gemini key via env + `gemini_key.txt` (the intended runtime workaround, so the key is never committed). But `tool_decision::LlmMayUsePredictor::from_env()` reads `FERRITE_GEMINI_API_KEY` ONLY — no file fallback — so if the env var is unset but `gemini_key.txt` is present, the agent runs keyed while the predictor silently runs keyless (empty may-use, rules-only fingerprinting). This is a config confound that would worsen M3 for reasons unrelated to the defense. Fix = one shared loader (env first, then file) used by both; warn (not fail) on a keyless predictor during eval runs. See `FINALIZED_DECISIONS.md` consequence 5.