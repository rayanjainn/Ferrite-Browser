# Ferrite Browser — Project Reference

> A comprehensive reference document for the Ferrite Browser project.
> This is a general-purpose reference — for the Claude Code CLI context file, see `CLAUDE.md`.

---

## 1. Project Overview

**Ferrite Browser** is a developer-focused, Rust-based web browser built on the Servo rendering engine. It is designed from the ground up to treat AI agents as first-class principals with formal capability-based access control and tamper-evident audit logs.

The project is a major academic submission (2026) developed by a three-person team over an 8-month timeline (24 person-months).

### Target Personas
- **"Agent Alex"** — AI/agent developers needing governance and auditability over LLM browser automation
- **"Pentester Pete"** — security researchers needing deep extension inspection
- **"Private Priya"** — privacy-conscious developers

### Core Differentiator
Capability-based agent governance: every privileged browser action requires an unforgeable capability token evaluated against composable Rego policies, recorded in a tamper-evident audit log. No other browser — including Perplexity Comet, ChatGPT Atlas, Dia, Brave, Chrome, or Edge — implements this.

### Three Core Deliverables
1. **Working browser prototype** — capability-brokered agent governance on Servo (Track A) and CEF (Track B)
2. **Two standalone developer tools** — Extension Security Analyzer (CLI) and Agent Action Inspector (UI)
3. **Research paper** — first formal model for capability-based governance of LLM browsing agents (targeting NDSS 2027 or ACSAC 2026)

---

## 2. The Problem

Browsers were designed 30 years ago for humans clicking links. In 2026, AI agents autonomously browse, fill forms, make purchases, and execute complex multi-step workflows inside the same architecture. The result is a governance vacuum.

Agentic browsers like Comet, Atlas, and Dia grant AI agents the same privileges as the human user with no formal governance model, no audit trail, and no capability-based access control. Traditional browser security (same-origin policy, sandboxing) becomes irrelevant when an AI agent executes with full user privileges.

OpenAI admitted in December 2025 that prompt injection "may never be fully solved" at the model level — making architectural enforcement essential.

### Real Attacks in 2025
- **CometJacking** — exfiltrated emails and calendar data via hidden instructions (LayerX, Oct 2025)
- **Screenshot Injection** — hidden text hijacked Comet and Fellou agents (Brave, Oct 2025)
- **Tainted Memories** — poisoned Atlas's long-term memory via CSRF (LayerX, Oct 2025)
- **HashJack** — instructions hidden in URL fragments (Cato Networks, Nov 2025)

---

## 3. Research Gaps Ferrite Fills

Academic research has advanced browser security and prompt injection defenses in parallel, but no prior work bridges them into a unified architectural solution for agentic browsing.

| Research Area | Key Works | Gap Ferrite Fills |
|---------------|-----------|-------------------|
| Browser Compartmentalization | Capsicum (USENIX 2010), COWL (OSDI 2014), RLBox (USENIX 2020) | None extended to agentic contexts where AI acts with user privileges |
| Prompt Injection Research | Greshake et al. 2023, Nasr et al. Oct 2025 | All defenses are model-level. No architectural enforcement exists |
| Browser Agent Benchmarks | WASP, BrowseSafe, SecureWebArena (2025) | Benchmarks describe the problem but propose no architectural defense |
| Transparency & Auditability | Certificate Transparency (RFC 6962), Sigstore/Rekor | Never applied to browser agent action auditability |

**Core research gap:** No system provides capability-mediated governance of browser agent actions with cryptographic verifiability. Ferrite fills this by combining the Capsicum/COWL lineage with CT-style audit logs.

---

## 4. Architecture Summary

Ferrite runs as multiple logical components within a single OS process (using Tokio async tasks). Process isolation is future work.

### Trusted Zone
- **UI Shell (Iced)** — browser chrome, consent dialogs, custom DevTools panels
- **Capability Broker** — central authority: mints tokens, validates tokens, evaluates policies, classifies risk
- **Policy Engine (Regorus)** — evaluates Rego policies against capability request context
- **Risk Classifier** — categorizes requests as low/medium/high risk
- **Audit System** — tamper-evident log with hash chain (upgrading to Merkle tree at month 5)

### Untrusted Zone
- **Servo Engine (Track A)** — renders web content via SpiderMonkey + WebRender/wgpu
- **Extension Sandbox (Extism/Wasmtime)** — runs Wasm extensions with capability-gated host functions
- **Agent Runtime** — accepts WebSocket connections, routes JSON-RPC commands through broker
- **Network Stack** — hyper + rustls + quinn + adblock-rust
- **CEF Harness (Track B)** — Chromium via cef-rs, proves engine-agnostic architecture

### Trust Boundary Crossing Rules
- Untrusted → Trusted: only via structured capability requests over async channels
- Trusted → Untrusted: only via opaque, unforgeable, scoped capability tokens
- Broker NEVER passes raw privileged handles across the boundary
- UI NEVER renders unsanitized untrusted content in consent dialogs

---

## 5. Six Security Gaps Ferrite Solves

| Gap | Existing Browsers | Ferrite's Solution |
|-----|-------------------|--------------------|
| Capability-based agent security | MV3 permission model (Chrome/Brave); no formal system (Comet/Atlas) | Broker-minted scoped tokens + WASI sandbox. Agents are first-class principals. |
| Verifiable agent audit trail | NetLog is diagnostic, not cryptographic. Comet/Atlas have no tamper-evident logs. | CT-style hash-chained → Merkle log. Cryptographic proof of every agent action. |
| Prompt injection hardening | Model-level training + classifiers — OpenAI admits insufficient. | Risk-tiered gating, step-up consent in trusted UI isolated from web content. |
| Extension Wasm sandboxing | Process isolation + permission prompts (MV3). Coarse-grained. | Per-capability, per-origin, rate-limited Wasm component grants. |
| Content blocking as primitive | Brave Shields blocks ads but not connected to formal audit. | adblock-rust engine with every block logged to cryptographic audit system. |
| Governed AX tree for agents | No existing browser provides governed accessibility tree access. | AX tree exposed only through capability-gated broker. First-of-kind. |

---

## 6. Technology Stack

### What We Build (Core Contribution)
1. **Capability Broker + Token System** — central enforcement point, pure Rust, zero external privilege dependencies
2. **Risk Classifier + Agent Governance** — Rego/OPA policy bundles via Regorus, risk tiers with step-up consent
3. **Verifiable Audit Log** — hash-chain evolving to Merkle tree with rs_merkle
4. **DevTools + Agent Protocol** — Capability Inspector, Agent Timeline, Audit Log Viewer (Iced), JSON-RPC over WebSocket

### What We Reuse (Open Source Foundation)
- **Browser Engine:** Servo v0.0.5 (MPL-2.0, pinned), CEF/Chromium (BSD, Track B)
- **Sandbox & Policy:** Extism/Wasmtime (Apache-2.0), Regorus (MIT)
- **Network & Crypto:** Tokio, hyper, rustls, quinn, rs_merkle, tracing + OpenTelemetry
- **Storage & UI:** rusqlite, keyring, adblock-rust (MPL-2.0), Iced (MIT)

---

## 7. Competitive Landscape (2025–2026)

| Browser | Key Features | Security Posture |
|---------|-------------|-----------------|
| Perplexity Comet | Chromium-based, shopping/booking/email tasks | Vulnerable: CometJacking, Amazon lawsuit. No capability model. |
| ChatGPT Atlas | RL-based red teaming + adversarial model updates | Admits prompt injection "unlikely to ever be fully solved." No crypto audit trail. |
| Dia (Atlassian) | Replaced Arc ($610M acquisition), AI-first with memory & skills | Lighter on security research. No formal capability disclosures. |
| Brave + Leo AI | 100M+ users, privacy-first with Shields | Developing "fine-grained permissions" — not yet shipped. |
| Chrome / Edge | Gemini + Copilot integrations | Published agentic security paper (Dec 2025). Incremental approach. |

**Key insight:** Every major player is adding AI agents to browsers. None have implemented formal capability-based access control or verifiable audit trails for agent actions.

---

## 8. Implementation Plan (Months 1–2 Focus)

### Month 1: Foundation

**Tasks:**
- `ferrite-types` crate — shared vocabulary (CapabilityToken, AuditEvent, CapabilityRequest, PolicyResult, RiskLevel, etc.)
- Servo v0.0.5 embedding shell — basic Iced window rendering a web page
- `ferrite-broker` standalone library — token minting, validation, policy evaluation via Regorus
- `ferrite-audit` — hash-chain append-only log writer + SQLite index for queries
- `ferrite-policy` — Regorus integration, policy file loading from `policies/` directory, risk classifier
- CI/CD pipeline (GitHub Actions) — build, test, clippy, fmt checks on all crates
- Docker devcontainer — reproducible build environment with all Servo dependencies
- Iced UI shell — basic browser window with tab bar, address bar, content area

**Milestone:** Servo renders a page. Broker passes unit tests. CI green. Verified by screenshot of rendered page, test report, CI badge.

**Dependencies:** ferrite-types blocks everything. Servo embedding and ferrite-broker are independent and can proceed in parallel.

### Month 2: Integration

**Tasks:**
- Broker ↔ Servo integration — request interception via Tokio async channels
- `ferrite-sandbox` — Extism setup, Wave 1 host functions (host_dom_read, host_network_fetch, host_storage_read, host_storage_write)
- Audit log viewer panel in Iced — displays events from audit system via broadcast channel
- Integration testing: extension loads in sandbox, calls host_dom_read through broker, receives DOM data

**Milestone:** Extension loads in sandbox. Broker integrated with Servo. Demo: extension calls host_dom_read and receives DOM data. Verified by demo recording.

**Dependencies:** Needs broker + Servo from month 1. Sandbox needs broker.

### Critical Path
```
ferrite-types → ferrite-broker → Servo integration → end-to-end capability loop
→ agent runtime → agent governance demo → adversarial evaluation → paper
```

### Risk-Adjusted Contingency Plans
| Risk | Trigger | Contingency |
|------|---------|-------------|
| Servo embedding takes >4 weeks | End of month 1, no rendered page | Pivot to CEF-primary. Servo becomes research track. |
| Extism proves too constraining | End of month 2, host functions don't work | Switch to raw Wasmtime immediately. Accept 3-week delay. |
| Servo upgrade at month 4 breaks things | >1 week on upgrade | Abort upgrade. Stay on original pin. |
| Agent LLM API costs >$200/month by month 4 | Budget exceeded | Switch to local LLM (Ollama + Llama 3). Reserve API for final eval. |
| Team member leaves/unavailable | Any time | Prioritize critical path items. Defer standalone tools and polish. |

---

## 9. Full Timeline Overview

| Month | Focus | Key Milestone |
|-------|-------|---------------|
| 1–2 | Foundation | Servo renders page, broker unit tests pass, CI green, extension loads in sandbox |
| 3 | Integration | End-to-end demo: extension blocked from unauthorized access, denial cryptographically logged |
| 4 | Agent Governance | LLM agent via JSON-RPC/WS, risk classification, CEF Track B, Wave 2 capabilities |
| 5 | Hardening | 20+ adversarial scenarios, extension security analyzer CLI, AX tree, Merkle tree upgrade |
| 6 | Evaluation | Three polished demos, broker latency measurement, proof verification, bandwidth savings |
| 7–8 | Release | Paper writing (NDSS 2027 / ACSAC 2026), reproducible artifact, open-source release |

---

## 10. Quantitative Targets

| Metric | Target |
|--------|--------|
| Enforcement coverage | 100% — no privileged action without valid token |
| Broker latency | <1ms median capability check overhead |
| Attack scenarios tested | 20+ including indirect prompt injection |
| Bandwidth savings | ≥10% on tracker-heavy sites via adblock-rust |

---

## 11. Future Work (Beyond 8 Months)

- Formal verification of capability token properties
- Public transparency log integration (Sigstore Rekor)
- Privacy-by-default partitioned state (per-origin storage isolation)
- Energy-aware scheduling with per-tab resource budgets
- Windows and Android platform support
- Process isolation (separate OS processes per component)

---

## 12. Licensing

Dual **MIT/Apache-2.0** license. Community contributions welcome from day one.

---

## 13. Team

Three-person team:
- **R1** — Systems/engine (Servo embedding, network stack, engine trait)
- **R2** — Security/crypto (broker, audit log, policy engine, adversarial testing)
- **R3** — UI/research (Iced shell, DevTools panels, paper writing)

---

## 14. Reference Documents

The canonical project knowledge base is an 8-file markdown reference document set:
- `00_master_index` — Document map and cross-references
- `01_objectives` — Project goals, personas, success criteria
- `02_architecture` — System-level architecture (source of truth)
- `03_capability_broker` — Token format, lifecycle, minting, validation
- `04_07_policy_sandbox_audit_governance` — Policy engine, Wasm sandbox, audit log, agent governance
- `08_threat_model` — Threat analysis and mitigations
- `09_12_ui_engine_network_a11y` — UI shell, engine integration, network stack, accessibility
- `13_15_plan_stack_evaluation` — Implementation plan, technology stack, evaluation methodology

These are stored in the parent `Major Project` folder and are designed to be self-contained — each section can be handed independently to any AI tool for context.
