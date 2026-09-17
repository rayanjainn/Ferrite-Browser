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

**Commits:** `e474403` (flatten) `a45b2ad` (delete dead crates) `d670dad`
(gitignore merge) `9e3f097`+`ffe3c63` (archive pass — the first attempt's
`git add -A -- docs/ Resources` silently failed on the already-moved
`Resources/` pathspec and committed pure renames with no banner content;
`ffe3c63` is the fixup that actually landed the banners and the three new
A0 files). Remaining commits for the CLAUDE.md/README/devcontainer
rewrites, hooks, and TO-DO.md regeneration are cited in
`docs/handoffs/a0.md`.

**Tests:** none — A0 is archaeology/doc/config work, not implementation.
`cargo metadata --no-deps` (before/after diff, see above) is the closest
thing to a test this phase has, and it's cited because R2 requires a
citation, not because it's a unit test.

**Known issues discovered, filed as T-### (see `docs/TO-DO.md`):**
- The commit that added the fixup above (`ffe3c63`) is itself evidence for
  why R2 ("no status claim without proof") matters: the first archive commit
  claimed to include content it didn't, and only got caught because this
  entry was written by re-deriving the diff rather than trusting the earlier
  commit message. No T-### filed — this is a demonstrated argument for R2,
  not a defect in the product.
