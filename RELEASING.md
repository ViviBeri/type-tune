# Releasing type-tune

This runbook covers the public npm release of the Rust CLI and its platform packages. The npm
launcher is published only after every native package is available, so users of `npx type-tune` do
not receive a launcher before its matching binary can be installed.

## One-time setup

Repository code and npm account settings are separate. The workflow in this repository authenticates
with GitHub Actions OIDC. It does not read `NPM_TOKEN`, `NODE_AUTH_TOKEN`, or any other long-lived
npm publishing token. Creating the Trusted Publisher records below is a manual npmjs.com change and
is not done by merging workflow changes.

Before a release, confirm that:

- the repository is public at [necoz-oss/type-tune](https://github.com/necoz-oss/type-tune);
- the GitHub `npm` environment exists and is the environment selected by the publishing jobs;
- repository metadata, package metadata, and the intended release version agree;
- every package below has its own Trusted Publisher configuration.

All six packages require this configuration. The root package and the scoped native packages may
belong to different npm users or organizations, so check ownership and permission to edit Trusted
Publishers on each package separately:

- `type-tune`
- `@type-tune/darwin-arm64`
- `@type-tune/darwin-x64`
- `@type-tune/linux-x64-gnu`
- `@type-tune/linux-x64-musl`
- `@type-tune/win32-x64-msvc`

For each package, open its npm package settings and add a Trusted Publisher with exactly these
values:

| Setting | Value |
| --- | --- |
| Provider | GitHub Actions |
| GitHub organization | `necoz-oss` |
| Repository | `type-tune` |
| Workflow filename | `npm-publish.yml` |
| GitHub Environment | `npm` |
| Allowed action | `npm publish` |

The workflow filename must match exactly, including the `.yml` extension. Select the allowed action
`npm publish`. Configurations created after 3 September 2026 start with staged publishing
(`npm stage publish`) and do not permit a direct publish until `npm publish` is selected. This
workflow runs `npm publish`, not `npm stage publish`.

npm does not validate every identity setting when the configuration is saved. A wrong organization,
repository, filename, or environment is reported only when a publish is attempted.

A new Trusted Publisher configuration must complete its first successful publication within two
days. If it does not, npm expires the configuration; delete it, create it again, and publish within
two days. After that first success, the configuration remains usable. Create the six configurations
when the next release can publish inside that window.

GitHub Actions OIDC authentication is independent of `npm whoami`. A successful local login does
not prove that the workflow can publish, and a missing local login does not block OIDC.

Publishing does not use a stored npm token. Revoke any leftover npm automation tokens, and do not
keep a GitHub Actions `NPM_TOKEN` secret. Do not put an npm token in repository files, workflow
logs, issue comments, or release notes.

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

The publishing jobs run on GitHub-hosted runners with `contents: read` and `id-token: write`. npm CLI
11.5.1 or later obtains a short-lived registry credential from GitHub Actions automatically. The
workflow still publishes with `--access public` and `--provenance`. Per-tag concurrency prevents
duplicate runs from publishing simultaneously. Publishing the five packages is not atomic: one native
package may already be published when another fails. Inspect each matrix job and npm package version
before retrying; successful publications are not rolled back. If any package was accepted, fix the
issue and release a new version rather than relying on unpublishing or retrying an immutable version.
Account ownership, npm environment approvals, repository visibility, and Trusted Publisher setup are
manual administrative prerequisites; the workflow cannot establish them.

`npm publish` returning successfully does not mean the version is immediately installable. Staged
publication and registry propagation can delay public installation. Wait for that delay before
concluding that a client install failed.

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

Confirm that the release used OIDC rather than a stored npm token:

1. Open the **NPM Publish** run for the tag and select each native publish job and the root publish
   job. The workflow file for that run must not set `NODE_AUTH_TOKEN` or `NPM_TOKEN`.
2. Each publish step must succeed. An `ENEEDAUTH` failure means the npm CLI did not accept the
   Trusted Publisher identity.
3. On each of the six npm package pages, open the new version and confirm its provenance names
   GitHub Actions, the release commit, and `.github/workflows/npm-publish.yml`.
4. Re-run the install checks above only after registry propagation. A successful publish log can
   precede a version that `npx` can install.

## If publishing fails

- **Before root publication:** inspect preflight, every native build, artifact verification and each
  publication matrix job in the failed workflow run. Query the registry for every platform package.
  Correct the failure; if any package was accepted, bump the version and restart from the release PR.
- **After root publication:** publish a patch release with the correction. Keep the affected version
  documented in the changelog and communicate any workaround in the GitHub release notes.
- **Checksum or launcher failure:** stop promotion, reproduce with a packed package in a clean
  directory, and verify the staged binary and `.sha256` file before issuing the patch.
