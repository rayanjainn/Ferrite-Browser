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
