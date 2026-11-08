# Native Apply Parity Through Compatibility Exports

**Captured:** 2026-10-04
**Context:** Extending the Rust `scan -> plan -> apply` core while its output must remain compatible with the distributed TypeScript CLI.
**Tags:** type-tune, rust, typescript, native, plan, apply, rewrite, parity, fixtures

## Problem

Moving several type declarations can preserve type correctness while still produce different public
module shapes or source text from the TypeScript implementation. The important cases are namespace
imports that rely on the original module namespace, and named imports from separate source modules
that converge on one destination module.

## Solution

Always leave a type-only compatibility re-export in the original source module. This lets namespace
imports continue to target the original module; only named imports are redirected to the destination.

When multiple named type-import statements in one referrer move to the same destination, merge only
contiguous statements whose bindings are all moved, type-only, and neither default nor namespace
imports. Replace their full statement range with one `import type { ... }` declaration. Preserve
aliases and quote style. Do not merge partial, mixed, or separated statements: retaining their
original declarations is safer than broad text replacement.

Destination rendering must match TypeScript's declaration boundaries: retain a single newline for
same-line declarations from the same source, and use a blank line when declarations originated on
different source lines or files.

## Example

```rust
if import.is_namespace {
    // The original module now re-exports the moved type, so its namespace remains valid.
    preserved_namespace_import = true;
    continue;
}
```

For a referrer containing separate imports from `alpha.ts` and `beta.ts`, both moved to
`types/src.ts`, replace the contiguous imports with:

```ts
import type { Alpha, Beta } from "./types/src";
```

## When To Use

- Adding Rust rewrite support for a new import strategy or destination layout.
- Changing source compatibility exports or destination declaration rendering.
- Handling multiple reviewed candidates in one native `apply` operation.
- Investigating a native/TypeScript differential result that is type-correct but textually different.

## Verification

Run focused Rust rewrite tests plus the native differential gate:

```sh
pnpm native:check
pnpm native:test
pnpm native:parity
```

Keep fixtures for relative imports, path aliases, barrels, namespace imports, NodeNext `.js`
specifiers, same-source multi-candidate moves, local dependencies, and multiple sources converging
on one destination.

## Related Files

- `crates/type-tune-core/src/rewrite.rs`
- `scripts/native-differential.mjs`
- `src/rewrite/referrer-imports.ts`
- `src/rewrite/apply-single-candidate.ts`
- `docs/rust-migration.md`
