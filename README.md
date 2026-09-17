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

This repository is **mid-rebuild**. `docs/REBUILD_DIRECTIVE.md` is the
controlling plan; `docs/PROGRESS.md` is the current, dated status (append-
only, cites tests and commits); `docs/TO-DO.md` is the live task ledger.
Do not trust this README, or any file under `docs/archive/`, for current
state beyond what's written in this section — read those three instead.

## Workspace

```
crates/
├── ferrite-shell     top-level binary, CLI dispatch
├── ferrite-servo     Servo engine integration (feature-gated: --features servo)
├── ferrite-ui        Iced UI: tabs, address bar, agent sidebar, consent panel
├── ferrite-agent     agent runtime: BrowserTool primitives, AgentRuntime trait, Gemini backend
├── ferrite-audit-log SHA-256 hash-chained, SQLite-backed audit log
├── ferrite-ipi       the IPI defense: fingerprinting, sanitizer, dry-run, comparator
└── ferrite-eval      evaluation harness (Servo-free) — dataset schema, adjudication, metrics
```

Servo is feature-gated and not built by default — most development and all
of the IPI defense logic never touches it. See `CLAUDE.md` for build
commands and hard invariants (in particular: no crate outside this list may
be reintroduced without explicit sign-off — see the "Never reference"
section there for what was deliberately deleted and why).

## Building

```
cargo build --workspace
cargo test --workspace
```

Building the real Servo engine is optional and slow:

```
cargo build -p ferrite-servo --features servo
```

A `justfile` single-command surface is planned (`docs/REBUILD_DIRECTIVE.md`
§6/A1) but not yet built — the commands above are the current, real way to
build and test this workspace.
