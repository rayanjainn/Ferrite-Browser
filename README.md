# Ferrite

Ferrite is a Rust agentic browser, built on the [Servo](https://servo.org/)
engine, that defends AI browser agents against **indirect prompt injection
(IPI)** architecturally rather than by trying to detect malicious text.

## The idea

An AI agent browsing the web reads content it doesn't control. Some of that
content can carry instructions aimed at the agent, not the user — a hidden
`<div>`, an HTML comment, an image `alt` attribute, a field in a tool's JSON
response. Ferrite doesn't try to win the arms race of spotting every phrasing
of an injected instruction. Instead it watches *what the agent does*:

1. **Predict** the task's expected fingerprint — which tools and origins the
   task legitimately needs — from the user's prompt, before any untrusted
   content is read.
2. **Dry-run** the agent's actual plan in a sandbox against synthetic,
   fake-but-plausible data, with no real network reachable.
3. **Compare** what the agent actually did against what was predicted.
4. **Consent-gate** anything beyond the prediction in a trusted UI surface,
   decoupled from page content, before the real run proceeds.

A successful injection can still make the agent *try* something unexpected —
but trying it during a contained dry-run against fake data, gated behind a
consent prompt the user controls, is a very different outcome than trying it
for real. A SHA-256 hash-chained audit log records the security-relevant
events so containment is verifiable after the fact, not just asserted.

## Status

The A0–A13 rebuild plan (`docs/REBUILD_DIRECTIVE.md`) is **complete** as of
A13 (reconciliation & release). That means every charter ran, every defect
in the D1–D14 register was fixed or explicitly ratified as intended
behavior, and `docs/EVALUATION.md` reports real numbers from a real (if
small) corpus with no fabricated data anywhere.

**It does not mean "feature-complete" or "ready to trust in production."**
Read `docs/EVALUATION.md` §5 and §6 before drawing any conclusion from the
eval numbers — in particular:

- The live `ferrite-ui`/`ferrite-shell` app still runs on the pre-rebuild
  agent path (`GeminiAgent`/`BrowserTool`/`ToolExecutor`), not the new
  `ferrite-model`/`ferrite-engine`/`browser_loop` stack this rebuild built
  and tested in isolation (`docs/TO-DO.md` T-224). The defense loop itself
  (`ferrite-ipi`) is real and tested; what runs when you launch the app is
  not yet wired to the new provider/engine layer.
- The eval corpus has **29 cases**, not the ~360 the methodology's own power
  calculation targets (`docs/TO-DO.md` T-227) — every confidence interval in
  `docs/EVALUATION.md` is correspondingly wide.
- `ServoEngine` (the new, engine-agnostic path's Servo backend) does not yet
  complete a real page load in this environment (`docs/TO-DO.md` T-220),
  though the live app's separate, older Servo integration
  (`ferrite-servo`/`ferrite-shell`) does — confirmed by direct observation.

`docs/PROGRESS.md` is the dated, cited status record; `docs/TO-DO.md` is the
live task ledger (open items, held items, and everything closed with the
commit that closed it); `docs/EVALUATION.md` is the full accounting,
including a `docs/REBUILD_DIRECTIVE.md` §14 definition-of-done checklist
scored item by item. Read those three for current state — not this section,
once time has passed since it was written.

## Workspace

```
crates/
├── ferrite-core           types, IDs, capability/primitive taxonomy, OriginScope, Clock
├── ferrite-model           ModelProvider trait: Mock/Replay/Ollama/Gemini + cache/throttle/budget decorators
├── ferrite-audit-log       SHA-256 hash-chained, SQLite-backed audit log
├── ferrite-ipi             the IPI defense: fingerprint, sanitizer, dry-run, twin, comparator
├── ferrite-engine          BrowserEngine trait + MockEngine (always built, no feature flag)
├── ferrite-engine-servo    ServoEngine, the BrowserEngine impl over real Servo (feature `engine-servo`)
├── ferrite-agent           BrowserTool/AgentRuntime/ToolExecutor (pre-rebuild, still load-bearing) + browser_loop (new, additive)
├── ferrite-servo           the live app's own Servo integration (HeadlessServoSession), feature `servo`
├── ferrite-ui              Iced UI: tabs, address bar, agent sidebar, consent panel
├── ferrite-shell           top-level binary, CLI dispatch
└── ferrite-eval            evaluation harness (Servo-free): dataset schema, corpus, harness, adjudication, metrics
```

Two things worth naming explicitly because they look like duplication and
aren't: `ferrite-engine-servo` and `ferrite-servo` are different, deliberate
things — the former is the new `BrowserEngine`-trait Servo backend (T-220:
real navigation doesn't complete yet), the latter is the live app's own,
working Servo integration that `ferrite-shell`/`ferrite-ui` actually run.
`ferrite-agent` similarly carries both the pre-rebuild `BrowserTool`/
`AgentRuntime` path (still what the live app runs, per T-224) and the new,
additive `browser_loop` module (engine/provider-agnostic, tested, not yet
wired into the live app). The target architecture in
`docs/REBUILD_DIRECTIVE.md` §4 also names a `ferrite-cli` crate; it was
never split out of `ferrite-shell` — `ferrite-shell` remains the one binary.

Servo (`ferrite-servo`'s `servo` feature, or `ferrite-engine-servo`'s
`engine-servo` feature) is not built by default — most development and all
of the IPI defense logic never touches it. See `CLAUDE.md` for build
commands and hard invariants (in particular: no crate outside this list may
be reintroduced without explicit sign-off — see the "Never reference"
section there for what was deliberately deleted and why).

## Building

The `justfile` is the single command surface (`just --list` to see every
recipe). Common ones:

```
just check   # fmt-check + clippy (--all-targets, deny warnings) + unused deps
just test    # full workspace test suite — no network, no API key required (R7)
just eval    # runs the real corpus through the harness, writes the metrics report
just audit   # cargo-deny: licenses, security advisories, duplicate versions
```

Plain `cargo` works too if you don't have `just`:

```
cargo build --workspace
cargo test --workspace
```

Building the real Servo engine is optional and slow (`just build-servo`, or
`cargo build -p ferrite-shell --features ferrite-servo/servo`).

### Local tooling

`rustup` (stable, with the `rustfmt`/`clippy`/`llvm-tools` components —
`rust-toolchain.toml` pins these) plus:

```
brew install just cargo-deny        # or: cargo install just cargo-deny
cargo install cargo-machete --locked
```

`just install-hooks` wires the commit-msg/pre-commit git hooks after that.
