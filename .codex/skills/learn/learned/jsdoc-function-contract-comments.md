# JSDoc Function Contract Comments

**Captured:** 2026-05-06
**Context:** When adding or changing functions in `type-tune-base`.
**Tags:** type-tune, typescript, jsdoc, typedoc, api-design, documentation, maintainability

## Problem

Functions in scanner, project loading, command, and rewrite modules encode important project contracts. Without consistent JSDoc, it is harder to understand which inputs are expected, which paths are normalized, and which return values must remain deterministic.

## Solution

Add a JSDoc block above each named function and constructor that explains the behavior and includes standard tags for the function contract.

Use `@param` for each parameter and `@returns` for the return value. Keep the summary and tag descriptions short, and describe repository-specific behavior such as deterministic output, POSIX path normalization, or plan/rewrite responsibilities when that matters.

## Example

```ts
/**
 * Creates a deterministic candidate ID from a stable key.
 *
 * @param stableKey - Candidate stable key.
 * @returns First 12 hex characters of the stable key hash.
 */
export function createCandidateId(stableKey: string): string {
  return createHash(stableKey).slice(0, 12);
}
```

## When To Use

Use this whenever adding or modifying named functions in `src/**/*.ts`, including local helper functions that carry non-obvious scan, path, hash, command, or test fixture behavior.

## Related Files

- `src/project/load-project.ts`
- `src/scan/exported-declarations.ts`
- `src/utils/hash.ts`
- `src/commands/scan.ts`
