# TypeDoc Comments For Exported Models

**Captured:** 2026-05-06
**Context:** When adding or changing exported TypeScript model types in `type-tune-base`.
**Tags:** type-tune, typescript, typedoc, models, api-design, documentation

## Problem

Exported model types define the public shape of scan, plan, and rewrite data. Without consistent comments on those types and their fields, future command output, generated declarations, and API-facing docs become harder to understand and review.

## Solution

Add TypeDoc comments to exported types and to every field of exported object types. For object type fields, place a short `/** ... */` comment immediately above each property.

Keep comments concise and behavior-focused. Describe the field's role in `type-tune-base` rather than restating the TypeScript type.

## Example

```ts
/** Exported type or interface declaration discovered during project scan. */
export type ExportedTypeDeclaration = {
  /** Candidate id. */
  id: CandidateId;
  /** Stable declaration key used for rewrite dedupe and conflict detection. */
  declarationKey: string;
};
```

## When To Use

Use this whenever adding or modifying exported model types, command option/result types, plan file types, scan candidate types, rewrite action types, or exported object-shaped helpers.

## Related Files

- `AGENTS.md`
- `src/models/candidate.ts`
- `docs/design.md`
