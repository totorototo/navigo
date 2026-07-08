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

7. **Push**
   ```
   git push
   ```

## For a release (version bump)

After steps 1–5, and before committing:

- Bump `version` in `Cargo.toml` (minor = `0.X.0`, patch = `0.0.X`)
- Do **not** hand-edit `npm/*/package.json` — CI syncs it from `Cargo.toml`

Then:
```
git add Cargo.toml ...
git commit -m "chore: bump version to X.Y.Z"
```

**Important**: the version bump must be the *last* commit before tagging — do not let unrelated commits land between it and the tag.

```
git tag vX.Y.Z
git log --oneline -1 vX.Y.Z   # verify tag points at the right commit
git push && git push origin vX.Y.Z
```
