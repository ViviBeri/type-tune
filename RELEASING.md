# Releasing type-tune

This runbook covers the public npm release of the Rust CLI and its platform packages. The npm
launcher is published only after every native package is available, so users of `npx type-tune` do
not receive a launcher before its matching binary can be installed.

## One-time setup

Before the first release, confirm that:

- the repository is public at [necoz-oss/type-tune](https://github.com/necoz-oss/type-tune);
- the npm packages `type-tune` and the `@type-tune/*` platform packages are owned by the publishing
  account or organization;
- the GitHub `npm` environment exists and has the `NPM_TOKEN` secret;
- npm trusted publishing/provenance is configured according to the publishing account's policy;
- repository metadata, package metadata, and the intended release version agree.

The `NPM_TOKEN` is used by the existing workflow. Do not put an npm token in repository files,
workflow logs, issue comments, or release notes.

## Prepare a release

1. Start from an up-to-date `main` branch with a clean working tree.
2. Use the **Version Bump** workflow to create the release pull request. It updates synchronized
   package versions and runs `pnpm publish:check` before opening the PR.
3. Review the generated changes, including every native package version, then merge the release PR.
4. Run the **Changelog** workflow to create the release-notes pull request and merge it.
5. Run the checks locally if anything changed after the version-bump workflow:

   ```bash
   pnpm publish:check
   pnpm docs:check
   pnpm docs:build
   ```
6. Run the **Build native packages** workflow and confirm that all platform jobs pass before
   creating the release tag. This verifies the native packages without publishing them.

`pnpm publish:check` runs the Rust formatter, Clippy with warnings denied, all Rust tests, a
release build, native version synchronization, and package validation.

## Publish

1. Create and push a signed or otherwise approved `v<version>` tag matching `package.json`.
2. Watch the **NPM Publish** workflow. Its preflight rejects branch refs, mismatched versions,
   unsynchronized manifests, or a tag, event SHA and checkout that do not identify the same commit.
   It runs `pnpm publish:check`, release gate tests and documentation checks before native builds.
   For a manual run, select the matching `v<version>` tag as the workflow ref; branch refs are rejected.
3. All five platform builds, native smoke tests, staged-package checks and applicable packed-launcher
   checks must succeed. The workflow downloads and checks all five packed artifacts, their metadata,
   commit, versions and checksums before any publishing job starts. Those exact tarballs are then
   published as native packages; the root launcher publishes only if every native publication succeeds.
   Linux musl's packed Node launcher is not exercised by this workflow and needs a separate musl test.
4. Do not publish the root package manually while native-package jobs are still running. The root
   package declares those packages as optional dependencies, but its launcher requires the matching
   installed binary at runtime.

The publish workflow uses `npm publish --provenance`. Per-tag concurrency prevents duplicate runs
from publishing simultaneously. Publishing the five packages is not atomic: one native package may
already be published when another fails. Inspect each matrix job and npm package version before
retrying; successful publications are not rolled back. If any package was accepted, fix the issue and
release a new version rather than relying on unpublishing or retrying an immutable version. Account
ownership, credentials, npm environment approvals, repository visibility and trusted-publisher setup
are manual administrative prerequisites; the workflow cannot establish them.

## Verify the published release

After npm propagation, verify the version from a clean temporary directory on each available host:

```bash
npx --yes type-tune@<version> --help
npx --yes type-tune@<version> scan --project tsconfig.json
```

Check at least macOS arm64 or x64, Linux x64 GNU, and Windows x64. Validate Linux musl on a musl
environment when one is available. Confirm that an unsupported platform produces the launcher's
clear installation error rather than selecting another implementation.

Finally, inspect the npm package pages, GitHub Actions run, changelog, and GitHub release/tag before
announcing the version.

## If publishing fails

- **Before root publication:** inspect preflight, every native build, artifact verification and each
  publication matrix job in the failed workflow run. Query the registry for every platform package.
  Correct the failure; if any package was accepted, bump the version and restart from the release PR.
- **After root publication:** publish a patch release with the correction. Keep the affected version
  documented in the changelog and communicate any workaround in the GitHub release notes.
- **Checksum or launcher failure:** stop promotion, reproduce with a packed package in a clean
  directory, and verify the staged binary and `.sha256` file before issuing the patch.
