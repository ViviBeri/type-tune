# Fail-Closed Plan Review

**Captured:** 2026-08-24
**Context:** Changes to the `scan -> plan -> review -> apply` workflow and its CLI safety boundary.
**Tags:** type-tune, typescript, cli, plan, review, apply, markdown, vitest, safety

## Problem

Treating `plan.md` as the human review source is ineffective if candidates start as `apply`, omitted
Markdown candidate lines silently fall back to `plan.json`, or `apply` accepts JSON alone without an
explicit acknowledgement. Any one of those behaviors can turn incomplete review into a rewrite.

## Solution

Keep the review boundary fail-closed as one coordinated contract:

- Generate plan candidates with `ignore` as the CLI default.
- When reviewed Markdown is supplied, require it to contain every candidate ID from `plan.json`.
  Continue rejecting duplicate and unknown IDs as well.
- When Markdown is omitted, require an explicit `--accept-unreviewed` flag before using statuses from
  `plan.json`, including for `--dry-run`.
- Keep `--default-status apply` available for intentional preselection, but do not let it bypass the
  Markdown-or-explicit-acceptance requirement.

Test the three boundaries independently and retain an end-to-end CLI test that changes selected
Markdown entries from `ignore` to `apply` before applying.

## Example

```ts
.option("--default-status <status>", "Initial status for plan entries: apply or ignore", IGNORE_PLAN_STATUS)

if (!options.markdown && !options.acceptUnreviewed) {
  throw new TypeTuneError(
    "Apply requires --markdown <path> or explicit --accept-unreviewed.",
  );
}

const missingIds = plan.entries
  .map((entry) => entry.candidate.id)
  .filter((candidateId) => !markdownStates.has(candidateId));
```

## When To Use

Apply this pattern whenever plan statuses, Markdown parsing, review UI behavior, apply options, or
plan schema compatibility changes. Recheck all three boundaries together; loosening one can silently
invalidate the human-review guarantee documented by the CLI.

## Related Files

- `src/commands/plan.ts`
- `src/commands/apply.ts`
- `src/plan/merge-plan-markdown-state.ts`
- `src/commands/apply.test.ts`
- `src/cli/smoke.test.ts`
- `docs/design.md`
- `docs/requirements.md`
