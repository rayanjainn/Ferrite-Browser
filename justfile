# Ferrite — the ONLY documented command surface (docs/REBUILD_DIRECTIVE.md §7/T-101/T-013).
# OS-neutral by construction: every recipe below is a `cargo`/`just` command,
# no PowerShell-only or bash-only syntax, no `C:\...` paths. Run `just` alone
# (or `just --list`) to see this list from any shell on any platform.

# Shared, portable target dir (see .cargo/config.toml's comment for why this
# lives here and not in a hardcoded `build.target-dir`). Override per-
# contributor with a real CARGO_TARGET_DIR env var; this is only the default.
export CARGO_TARGET_DIR := env_var_or_default("CARGO_TARGET_DIR", home_directory() / ".cache" / "ferrite-target")

# List all recipes (default).
default:
    @just --list

# Full local CI-equivalent gate: format, lint, unused-deps, license/ban audit, tests.
# This is what `just check && just test` (the A1 exit-gate phrase) expands to
# when you want both halves in one shot.
ci: check test

# Structural/style gate: fmt-check + clippy (deny warnings) + unused deps.
# Does NOT build Servo (default feature set) and does NOT run cargo-deny
# (that's `just audit` — separated because it hits the network for the
# advisory DB and is slower).
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo machete

# Format the whole workspace in place.
fmt:
    cargo fmt --all

# Clippy alone, deny warnings, all targets (lib + bins + tests + examples —
# the old CI's `cargo clippy --workspace` without --all-targets never linted
# test code at all; see docs/TO-DO.md T-207 for what that hid).
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# License/advisory/duplicate-version/source audit. Networked (fetches the
# RustSec advisory DB) — kept separate from `check` for that reason.
audit:
    cargo deny check

# Full workspace test suite (unit + integration + doctests). Must pass with
# no network and no provider API key set (R7) — nothing in this workspace's
# tests currently needs either, and CI should stay that way.
test:
    cargo test --workspace

# Unit tests only (crate libs, no integration tests/doctests) — quick
# iteration loop.
test-fast:
    cargo test --workspace --lib

# Live-provider tests, #[ignore]'d by convention so `just test` never spends
# quota (R7). Nothing is marked #[ignore] yet in this workspace — this
# recipe is here for when A3's ModelProvider conformance suite adds them,
# not decorative.
test-live:
    cargo test --workspace -- --ignored

# Lists every tag the configured Ollama endpoint actually serves
# (docs/REBUILD_DIRECTIVE.md §10.2's startup preflight target). Networked;
# needs OLLAMA_API_KEY (or the OS keyring) unless FERRITE_OLLAMA_BASE_URL
# points at a local endpoint, and FERRITE_MODEL_SMALL/FERRITE_MODEL_MAIN
# set (T-213: no model name is ever a default). Not part of check/test/CI.
models:
    cargo run -p ferrite-model --example models

# One live Ollama Cloud round-trip, by hand — A3's exit gate. Same
# credentials/config as `models`. Not part of check/test/CI; this is the
# one target the directive explicitly asks a human to run once, not a
# thing `just test` should ever do (R7).
probe:
    cargo run -p ferrite-model --example probe

# Reports the on-disk response cache's size and the hit rate flushed by the
# last run (§10.3: "a low hit rate is a bug, investigate it"). Offline —
# only reads ~/.cache/ferrite-model/ (or $FERRITE_MODEL_CACHE_DIR), no
# network, no model config required. Safe to run anywhere, including CI.
cache-stats:
    cargo run -p ferrite-model --example cache_stats

# Records one live response into the committed fixture directory
# (crates/ferrite-model/tests/fixtures/model/) for ReplayProvider to serve
# in offline tests — §10.3: "these are committed." Usage:
#   just record ollama "your prompt"
#   just record gemini "your prompt"
# Networked; same config/credentials as `models`/`probe`. Not part of
# check/test/CI — a fixture this writes is reviewed and committed by hand,
# like any other change to the test tree.
record provider prompt="":
    cargo run -p ferrite-model --example record -- {{provider}} {{prompt}}

# Run the shell binary (launches the Iced UI by default — see
# ferrite-shell/src/main.rs's CLI dispatch for the other subcommands:
# window, jstest, agent-smoke, smoke).
run *ARGS:
    cargo run -p ferrite-shell -- {{ARGS}}

# Build with the real Servo engine — feature-gated, slow, NOT part of
# `just check`/`just test`/CI's default path (docs/REBUILD_DIRECTIVE.md
# §7.1: Servo is optional and off by default; weekly-only in CI). Cost
# should be recorded in docs/BUILD_BUDGET.md whenever this is run.
build-servo:
    cargo build -p ferrite-shell --features ferrite-servo/servo

# Dependency-bloat report. Requires `cargo install cargo-bloat` (not
# bundled — it's a diagnostic tool you reach for before adding a
# dependency, per §7.3, not a gate every run needs).
bloat *ARGS:
    cargo bloat --release {{ARGS}}

# Run the evaluation harness. NOT YET IMPLEMENTED — the batch corpus
# driver + metrics aggregator is A12's deliverable (docs/TO-DO.md T-112).
# This recipe exists now (per the charter's command-surface list) but is
# honest about not doing anything yet rather than silently no-op'ing.
eval:
    @echo "not yet implemented — see docs/TO-DO.md T-112 (A12: Harness + metrics)" && exit 1

# Prune stale (7+ day old) build artifacts from the target dir. Requires
# `cargo install cargo-sweep` (not bundled, same reasoning as `bloat`).
# Deliberately NEVER a blanket `cargo clean` — REBUILD_DIRECTIVE.md §7.4.
clean-cache:
    cargo sweep -t 7 "$CARGO_TARGET_DIR"

# Report target-dir size, per-profile breakdown, and the 20 largest
# artifacts. No extra tool required — pure du/find/sort. See
# docs/BUILD_BUDGET.md for the target (<12GB, <5min cold `just test`
# without Servo) and the recorded numbers at each phase gate.
disk:
    @echo "=== target dir: $CARGO_TARGET_DIR ==="
    @du -sh "$CARGO_TARGET_DIR" 2>/dev/null || echo "(not built yet)"
    @echo ""
    @echo "=== per-profile ==="
    @du -sh "$CARGO_TARGET_DIR"/*/ 2>/dev/null || true
    @echo ""
    @echo "=== 20 largest artifacts ==="
    @find "$CARGO_TARGET_DIR" -type f -exec du -h {} + 2>/dev/null | sort -rh | head -20 || true

# Install the git hooks for this repo (commit-msg guard + pre-commit
# fmt/clippy gate). Idempotent — safe to re-run.
install-hooks:
    ./scripts/hooks/install.sh
