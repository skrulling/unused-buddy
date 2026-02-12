# Releasing unused-buddy

This project uses:

- GitHub Releases as the canonical binary source
- npm Trusted Publishing (OIDC) for `unused-buddy` and platform packages

## Prerequisites

1. Ensure npm package entries exist:
   - `unused-buddy`
   - `unused-buddy-darwin-arm64`
   - `unused-buddy-darwin-x64`
   - `unused-buddy-linux-arm64-gnu`
   - `unused-buddy-linux-x64-gnu`
   - `unused-buddy-win32-x64`
2. Configure Trusted Publisher in npm for each package:
   - Provider: GitHub Actions
   - Repo: this repository
   - Workflow file: `.github/workflows/npm-publish.yml`
   - Trigger context matching release tags
3. Trusted publishing must be enabled before tagging a release.

## Release flow

1. Update version in `Cargo.toml` to the target release version.
2. Commit and push changes.
3. Create and push a stable semver tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

4. `Release Binaries` workflow builds platform binaries and publishes a GitHub Release with:
   - platform archives
   - `checksums.txt`
   - `asset_manifest.json`
5. `Publish npm Packages` workflow runs on release publication and:
   - verifies OIDC and toolchain requirements
   - verifies archive checksums
   - publishes platform packages first
   - publishes `unused-buddy` meta package last

## Failure policy

- No `NPM_TOKEN` fallback is used.
- If trusted publishing is misconfigured, npm publish fails hard.
- Fix trusted publisher config and rerun the workflow.

## Local dry run for npm pack logic

If you have release assets locally:

```bash
./scripts/npm/pack-local.sh /path/to/release-assets v0.1.0
```

This performs `npm publish --dry-run` for generated packages.

## Release helper

You can use the built-in helper:

```bash
npm run release -- 0.2.0
```

It will:

1. ensure clean git working tree
2. ensure the tag does not already exist
3. run tests (unless `--skip-tests`)
4. bump `Cargo.toml` package version
5. commit (`release: vX.Y.Z`)
6. create tag (`vX.Y.Z`)
7. push branch and tag to `origin`

Dry-run mode:

```bash
npm run release -- 0.2.0 --dry-run
```
