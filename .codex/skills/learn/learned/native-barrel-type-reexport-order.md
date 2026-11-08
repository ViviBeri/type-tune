# Preserve standalone type barrel re-export order

**Captured:** 2026-10-21
**Context:** Rust `apply` rewrite parity with TypeScript printer output.
**Tags:** rust, rewrite, parity, barrel, re-export

## Problem

When rewriting a moved declaration through a barrel module, TypeScript keeps value
re-exports before a standalone type-only re-export from the same module. Emitting
the rewritten type export in its original earlier position produces semantically
equivalent code, but breaks deterministic native/TypeScript apply parity.

## Solution

For a standalone `export type { Name } from 'module';` with one export specifier,
check for a later value re-export. If one exists, delete the type-only statement
(including its adjacent line break) and insert the rewritten statement after the
last value re-export. Preserve the module-specifier quote style.

Apply this only to whole, standalone type-only re-export statements. Mixed inline
exports use the existing split-rewrite path and should not be reordered by this
rule.

## Example

Before a declaration move:

```ts
export type { Shared as default } from './shared';
export { value } from './shared';
```

After TypeScript-compatible rewriting:

```ts
export { value } from './shared';
export type { Shared as default } from './shared';
```

## When to use

Use this rule when a Rust source rewrite changes a barrel re-export and exact
TypeScript printer parity matters. Add a differential fixture with a default type
re-export before a value re-export from the same source.

## Related files

- `crates/type-tune-core/src/rewrite.rs`
- `scripts/native-differential.mjs`
