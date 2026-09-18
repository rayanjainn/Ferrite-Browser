# Build Budget

Targets per `docs/REBUILD_DIRECTIVE.md` §7.4: **< 12 GB target-dir** and
**< 5 min cold `just test` without Servo**. Record real numbers here at
every phase gate — this file is data, not aspiration; a >20% regression
should be fixed before moving on, per §7.4.

Machine for all measurements below: macOS (Apple Silicon, arm64),
`rustc 1.98.1`, cold numbers taken with `~/.cache/ferrite-target` deleted
but the cargo registry/index cache warm (a true from-network-zero clone
was not measured this session — see the caveat below).

## 2026-09-18 — A1 (Foundation), post workspace.dependencies + profiles + deny.toml + justfile

| Measurement | Value | Command |
|---|---|---|
| Dev-profile target-dir size (full workspace, all tests built) | **1.7 GB** | `just disk` after `just check && just test` from an empty target dir |
| Cold `just check && just test` (target dir wiped, registry cache warm) | **3m 32s** wall clock | `rm -rf ~/.cache/ferrite-target && time (just check && just test)` |
| Warm `just test` (nothing changed since last build) | **7.2s** | `time just test` |
| Release build, default features (no Servo) | **~23s** warm / **~2min** cold | `cargo build --release -p ferrite-shell` |

**Against the targets:** disk (1.7 GB) is well under 12 GB; cold
check+test (3m32s) is under the 5-minute bar. Both pass today, with
headroom — expected, since A1 hasn't yet added `ferrite-core`,
`ferrite-model`, or the corpus/eval machinery those later phases bring.
Re-measure at every subsequent phase gate; the headroom will shrink.

**Caveat on "cold":** this run reused the local cargo registry/index
cache (crates already downloaded, just not yet compiled into this
target dir). A genuine zero-state clone also pays crate-download time,
which depends on network speed and isn't reproducible from this
sandbox in a meaningful way — CI's `Swatinem/rust-cache@v2` step
(`.github/workflows/ci.yml`) is the real measurement surface for that;
its timing isn't available locally since this environment can't run
GitHub Actions (see `docs/PROGRESS.md`'s 2026-09-18 A0 entry for the
same limitation hit earlier, with a Docker build).

**Dependency hygiene applied this phase (T-101):** `tokio` trimmed from
`features = ["full"]` to `["rt-multi-thread", "macros", "time", "sync"]`
(dropped `parking_lot`/`signal-hook-registry` from the lock, per the
`process`/`signal` features nothing here uses); `chrono` trimmed to
`["clock", "serde"]`; one genuinely unused dependency removed
(`ferrite-ui`'s `uuid`, caught by `cargo machete`). The ~30-crate
duplicate-version list `cargo deny` reports is NOT a disk-reduction
opportunity available to this workspace today — it's the structural
cost of depending on Servo pinned to an old git tag alongside the
current iced/winit graphics stack (see `deny.toml`'s `[bans].skip`
comment). That cost only goes away if Servo is bumped or dropped,
neither of which is in scope here.

**Not yet measured (blocked on later phases / out of this session's
scope):** `just build-servo`'s actual cost — A9's exit gate, not A1's;
the directive's own estimate ("tens of GB and 30-60 minutes on first
build") is unverified against this specific pinned tag and this
machine. Do that measurement when A9 runs, not before — building
Servo now would cost real time for a number this phase doesn't need.
