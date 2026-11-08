# AI review

AI review is an optional advisory step in `type-tune plan`. It can help a reviewer assess whether a
candidate belongs in a shared type module, but it never changes source code and never replaces the
deterministic scan, plan, or apply logic.

## Enable it deliberately

AI review is disabled by default. Enable it only for a plan that you are comfortable sending to the
selected provider:

```bash
type-tune plan \
  --project tsconfig.json \
  --out .type-tune/plan.json \
  --markdown .type-tune/plan.md \
  --ai-review \
  --ai-provider openai \
  --ai-model <model>
```

Supported provider names are `mock`, `openai`, `openai-compatible`, `anthropic`, and `ollama`.
Provider-specific endpoint, model, and credential settings are supplied through the corresponding
`--ai-endpoint`, `--ai-model`, and `--ai-api-key-env` options. The API-key option names an
environment variable; it does not put its value in a plan file or command argument.

Use `--ai-dry-run` to inspect the normalized provider-neutral request without contacting a provider.
Use `--ai-request-out <path>` to save that request for review. The `mock` provider with
`--ai-response-file <path>` supports deterministic local tests and offline experiments.

## Review contract

The Rust core builds a request from an already deterministic plan. Each candidate contains the
stable candidate ID, declaration kind and name, source and proposed destination paths, referrer
paths and import strategies, local type dependencies, and a source fingerprint.

A provider must return a decision for known candidate IDs with:

- `status`: `apply`, `ignore`, or `needs-review`;
- `confidence`: a number from 0 through 1;
- `reason`: a human-readable explanation;
- `concerns`: a list of follow-up risks or checks.

The core validates response shape, candidate IDs, statuses, and confidence before merging a result
into a plan. Invalid data is never treated as an instruction to rewrite source. The merged decision
is an annotation: a person still controls the reviewed plan, and `apply` makes no AI request.

## Context and privacy

`--ai-context` selects how much source context is supplied:

| Mode | Data sent |
| --- | --- |
| `summary` | Candidate metadata, paths, dependency names, and import strategies. |
| `snippets` | Summary plus the candidate declaration, local type dependencies, and relevant referrer imports. |
| `full` | Snippets plus the involved source and referrer file text. |

The default mode is `snippets`. Use `summary` where source disclosure must be minimized. Before
enabling external review, assess the chosen provider's retention, regional processing, and access
controls for your project. Do not use it for code that your organization is not allowed to transmit.

`--ai-max-candidates` limits the number of candidates submitted. `--ai-max-context-bytes` bounds
source context; the generated request records when context was truncated so the reviewer can see
that the model had incomplete information. The request uses project-relative paths rather than
absolute filesystem paths.

## Failure behavior

The default `--ai-on-error fail` stops plan generation when a provider call or response validation
fails. Two explicit alternatives are available:

- `skip` leaves the deterministic plan unchanged when AI review fails.
- `mark-needs-review` writes a review annotation that calls out the failed AI check.

Use the default for CI or controlled review pipelines where an unavailable provider should be
visible. Use `skip` or `mark-needs-review` only when a usable deterministic plan is more important
than obtaining an AI opinion.

`--ai-timeout-ms` sets the provider request timeout; it must be positive. `--ai-cache <path>` stores
validated decisions keyed to the request and candidate context, avoiding unnecessary repeat calls
when the relevant plan inputs have not changed.

## Safe workflow

1. Run `scan` and inspect its deterministic output.
2. Generate a plan, optionally with `--ai-dry-run` first.
3. Read the Markdown plan, including any AI reasons and concerns.
4. Mark only the candidates you approve as `apply`.
5. Run `apply --dry-run` and inspect the reported changes.
6. Run `apply` only after the dry run is satisfactory.

AI review improves the information available to the human reviewer; the transaction, TypeScript
validation, and rollback mechanisms remain the safety boundary. See [Requirements](requirements.md)
for the complete apply contract.
