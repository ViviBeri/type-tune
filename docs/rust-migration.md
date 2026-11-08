# Rust CLI and npm distribution

## Status

The CLI migration is complete: all command behavior lives in Rust. JavaScript remains only at the
npm distribution boundary and in the independent Astro documentation site.

type-tune's CLI implementation is Rust-only. `crates/type-tune-core` owns project scanning,
planning, review-state handling, rewriting, transactional file changes, compiler validation, and AI
review integration. `crates/type-tune-cli` exposes those capabilities as the `type-tune` binary.
See [Architecture](design.md) for the command-level design.

## npm distribution

The `type-tune` npm package remains the supported `npx type-tune` entrypoint. It contains a small
JavaScript launcher and declares an optional native package for each supported platform. The launcher
selects the matching package, verifies the adjacent SHA-256 checksum when one is included, and then
executes its Rust binary. It has no command implementation and no fallback CLI.

If a matching native package is unavailable, the launcher exits with an installation error rather
than silently switching implementations. Set up another supported platform package or reinstall the
root package to resolve it.

Supported native packages are:

- macOS arm64 and x64
- Linux x64 GNU and musl
- Windows x64 MSVC

The root package has no JavaScript or TypeScript CLI fallback. This makes an installed version
unambiguous: the same Rust implementation runs from a source checkout, a direct binary, and
`npx type-tune`.

## Package integrity

Each native package contains the platform binary and, when staged for publication, an adjacent
SHA-256 checksum. Before execution, the launcher verifies that checksum when present. A missing
native dependency, unsupported host, or failed checksum is an actionable installation error rather
than an attempt to execute a different implementation.

## Development boundaries

The repository still uses Node.js and pnpm for the npm distribution scripts and for the Astro site.
That does not make them part of the CLI runtime implementation:

- Rust source, tests, formatting, linting, and release builds live in the Cargo workspace.
- `bin/type-tune.mjs` only resolves and launches an installed native package.
- `docs/` retains its Astro and TypeScript tooling independently of the CLI.

## Development and release checks

Run the complete implementation check with:

```bash
pnpm check
```

This runs `cargo fmt --check`, Clippy with warnings denied, all Rust tests, and a release build.
`pnpm package:check` verifies that the npm launcher and optional platform-package metadata are ready
to publish. `pnpm publish:check` additionally checks native package version alignment.

Release CI builds each target, stages its binary and checksum, and verifies the package contents.
On hosts that can run the target, it also installs packed npm artifacts into a temporary consumer
and invokes the root launcher. The Linux musl target is built and packaged on the GNU runner, but
the root launcher smoke test is intentionally skipped there because that runner cannot select the
musl package.

Native packages publish before the root `type-tune` package. This ordering ensures that a newly
published launcher can resolve its optional platform dependency immediately.
