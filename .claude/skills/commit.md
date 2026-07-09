# Commit skill for navigo

Run this before every commit and push. Stop at the first failure — do not proceed.

## Steps

1. **Format check**
   ```
   cargo fmt --check
   ```
   If it fails, run `cargo fmt` to fix, then re-check before continuing.

2. **Lint**
   ```
   cargo clippy --all-targets --all-features
   ```
   Zero warnings — CI uses `-D warnings`. Fix any warning before continuing.

3. **Tests**
   ```
   cargo test --all-features
   ```
   All tests must pass.

4. **Coverage**
   ```
   cargo llvm-cov --fail-under-lines 95 --fail-under-regions 90
   ```
   Must meet the 95% line / 90% region gates.

5. **READMEs**
   Check that `README.md`, `demo/README.md`, and `npm/README.md` reflect the change — no references to removed APIs, new public API documented. If anything is stale, update before committing.

6. **Commit**
   Use conventional commit format: `type(scope): description`
   - Types: `feat`, `fix`, `refactor`, `chore`, `test`, `docs`, `style`, `perf`
   - Scope is optional but encouraged (e.g. `wasm`, `gpx`, `cli`)
   - Subject line: imperative mood, no period, ≤72 chars
   - No `Co-authored-by` trailers

7. **Push to branch and open PR — never push directly to `main`**
   `main` is protected. Always work on a short-lived branch:
   ```
   git checkout main && git pull
   git checkout -b fix/<short-description>   # or feat/ docs/ ci/ chore/
   # … make changes, run steps 1–6 …
   git push -u origin fix/<short-description>
   # open PR on GitHub → wait for green CI → merge
   ```

## Releasing

**Never bump `Cargo.toml` or create tags manually.**

release-please handles this automatically. After your PR is merged to `main`:
- release-please opens (or updates) a "chore: release X.Y.Z" PR
- Merging that PR bumps `Cargo.toml`, writes `CHANGELOG.md`, creates the tag,
  and triggers crates.io / npm publish and Netlify deploy

The version bump is determined by your conventional commit prefix:
- `fix:` → patch, `feat:` → minor, `feat!:`/`fix!:` → minor (while `< 1.0`)
