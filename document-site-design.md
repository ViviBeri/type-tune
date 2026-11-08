# type-tune Document Site Design

> Scope: this is the implementation/design reference for the Astro documentation site. It does not
> describe the CLI implementation; use the [README](README.md) and
> [Rust architecture](docs/design.md) for that.

## Goal

The documentation site lives under `docs/` in this monorepo. This document records its intended
architecture and information design: help TypeScript developers quickly understand what `type-tune`
does, install it, run the scan-plan-apply workflow, and inspect available CLI configuration. The CLI
itself is Rust; Astro and TypeScript remain the documentation site's tooling.

The first version should be small, fast, and documentation-focused. It should not become a marketing site before the core docs are useful.

## Reference

Use `../is-kit/docs` as the main layout reference.

Reference patterns to carry over:

- Fixed top header with project name/logo, navigation links, and GitHub link.
- Sidebar layout for documentation pages.
- Landing page sections for hero, install commands, usage examples, and links.
- Centralized constants for navigation, install snippets, and page metadata.
- Reusable code block, code tabs, heading, paragraph, and link components.
- Mobile sidebar behavior with a header toggle.

Adaptation:

- `../is-kit/docs` is implemented with Next.js.
- `type-tune` docs will use Astro.
- Keep the visual and information architecture principles, but use Astro-native pages, layouts, content collections, and islands only when interactivity is needed.

## Tech Stack

- Framework: Astro
- Language: TypeScript
- Styling: Tailwind CSS or the same token-based CSS approach used by `../is-kit/docs`
- Content: Astro pages plus Markdown or MDX for long-form docs
- Package location: `docs/`
- Monorepo workflow: root package manages docs scripts through workspace filters

Recommended package scripts:

```json
{
  "docs:dev": "pnpm --filter ./docs dev",
  "docs:build": "pnpm --filter ./docs build",
  "docs:preview": "pnpm --filter ./docs preview"
}
```

## Site Map

```text
/
/usage
/config
```

Initial navigation:

- Home
- Usage
- Config
- GitHub

GitHub URL:

```text
https://github.com/necoz-oss/type-tune
```

## Astro + FSD Structure

Use a pragmatic FSD-style structure inside `docs/src`.

```text
docs/
  astro.config.mjs
  package.json
  tsconfig.json
  public/
    type-tune-logo.png
  src/
    app/
      styles/
        global.css
      providers/
        site-shell.astro
    pages/
      index.astro
      usage.astro
      config.astro
    widgets/
      site-header/
        site-header.astro
      docs-sidebar/
        docs-sidebar.astro
      home-hero/
        home-hero.astro
      install-tabs/
        install-tabs.astro
      usage-flow/
        usage-flow.astro
      config-table/
        config-table.astro
    features/
      copy-command/
        copy-command.ts
      sidebar-toggle/
        sidebar-toggle.ts
    entities/
      cli-option/
        model.ts
      docs-page/
        model.ts
    shared/
      config/
        navigation.ts
        install-commands.ts
        cli-options.ts
      ui/
        code-block.astro
        code-tabs.astro
        heading.astro
        text-link.astro
      lib/
        cn.ts
```

FSD boundaries:

- `app`: global shell, styles, base providers.
- `pages`: Astro route files only. Keep page files as composition layers.
- `widgets`: page sections and layout blocks.
- `features`: small interactive behavior, such as copy buttons or sidebar toggle scripts.
- `entities`: typed models for docs data such as CLI options and page metadata.
- `shared`: reusable UI, constants, and pure helpers.

## Layout Design

### Global Shell

Every page uses one top-level shell.

Responsibilities:

- Render fixed header.
- Reserve top padding for fixed header.
- Provide consistent max-width and responsive gutters.
- Set page title and description.

### Header

Inspired by `../is-kit/docs/components/navigation/site-header.tsx`.

Content:

- Left: sidebar toggle on docs pages, type-tune logo or text mark.
- Center/right: Home, Usage, Config.
- Right: GitHub icon/link.

Behavior:

- Fixed at top.
- Subtle border and translucent background.
- On mobile, navigation collapses into a sidebar toggle.

### Sidebar

Inspired by `../is-kit/docs/components/layout/sidebar-layout.tsx`.

Usage:

- Home can be full-width without sidebar.
- Usage and Config use sidebar layout.

Sections:

```text
Guide
  Why type-tune
  Workflow
  Usecases

Config
  Scan options
  Plan options
  Apply options
  Extract options
  AI review options
```

## Page Design

## Home

Route:

```text
/
```

Purpose:

Give a quick answer to "what is type-tune, why should I care, and how do I try it?"

Sections:

1. Hero
2. Install
3. Short usage
4. Other links
5. GitHub link

### Home: Short Description

Tagline:

```text
Move shared TypeScript types without breaking imports.
```

Catch copy:

```text
type-tune scans exported type aliases and interfaces, writes a reviewable plan, updates type-only imports, and validates the result with TypeScript before keeping changes.
```

Tone:

- Calm and practical.
- Emphasize reviewability and deterministic output.
- Avoid overstating automation. The tool helps with careful refactoring.

Primary actions:

- Get started
- View usage
- GitHub

### Home: How To Install

Use tabs or segmented controls for package managers, following the `is-kit` install section pattern.

Commands:

```bash
pnpm add -D type-tune
```

```bash
yarn add -D type-tune
```

```bash
bun add -d type-tune
```

```bash
npm install -D type-tune
```

Also show one-off usage:

```bash
npx type-tune scan --project tsconfig.json
```

### Home: Short Usage

Show the smallest useful workflow:

```bash
type-tune scan --project tsconfig.json
```

```bash
type-tune plan \
  --project tsconfig.json \
  --out .type-tune/plan.json \
  --markdown .type-tune/plan.md
```

```bash
type-tune apply \
  --plan .type-tune/plan.json \
  --markdown .type-tune/plan.md \
  --dry-run
```

```bash
type-tune apply \
  --plan .type-tune/plan.json \
  --markdown .type-tune/plan.md
```

### Home: Other Links

Link group:

- Usage guide
- Configuration reference
- Changelog
- Contributing
- Security policy
- npm package

### Home: GitHub Link

Place GitHub link in:

- Header.
- Hero action row.
- Footer link list.

## Usage

Route:

```text
/usage
```

Purpose:

Explain the problem, the workflow, and concrete scenarios.

Sections:

1. Why do you need to use type-tune
2. How to use type-tune
3. Usecases

### Usage: Why Do You Need To Use type-tune

Main points:

- Shared TypeScript types often begin near the first feature that needs them.
- Over time, those types become imported by unrelated files.
- Moving them manually means updating imports, checking local type dependencies, preserving aliases or barrels, and validating compile output.
- type-tune automates the mechanical rewrite while keeping the decision in a reviewable plan.

Suggested copy:

```text
When a type becomes shared, the hard part is rarely deciding where it should live. The hard part is moving it without missing an import, breaking a barrel export, or losing a local type dependency. type-tune handles that mechanical work through a plan you can review before applying.
```

### Usage: How To Use type-tune

Workflow:

1. Scan candidates.
2. Create a plan.
3. Review `plan.md`.
4. Preview with `apply --dry-run`.
5. Apply.
6. Commit the result.

Detailed command examples:

```bash
type-tune scan --project tsconfig.json --verbose --explain
```

```bash
type-tune plan \
  --project tsconfig.json \
  --out .type-tune/plan.json \
  --markdown .type-tune/plan.md
```

```md
- [apply] abc123def456 `OrderSummary`
```

```bash
type-tune apply \
  --plan .type-tune/plan.json \
  --markdown .type-tune/plan.md \
  --dry-run
```

```bash
type-tune apply \
  --plan .type-tune/plan.json \
  --markdown .type-tune/plan.md
```

Explain that `plan.md` is the human review surface and `plan.json` is the structured apply input.

### Usage: Usecases

Usecase cards or sections:

1. Feature type became shared
   - Example: `OrderSummary` starts in a page/component file and is now imported by API, UI, and tests.
   - Recommended: `scan -> plan -> dry-run -> apply`.

2. Move a specific symbol
   - Use `extract` when you already know the symbol.

```bash
type-tune extract OrderSummary \
  --from src/features/order/schema.ts \
  --to src/features/order/types/order.ts \
  --dry-run
```

3. Preserve path aliases
   - Explain that aliases are derived from `compilerOptions.baseUrl` and `compilerOptions.paths`.

4. Keep simple barrel imports working
   - Existing simple `index.ts` barrel imports can be preserved by updating re-exports.

5. Exclude generated files
   - Generated files are excluded by default.
   - Use `--include-generated` only when intentional.

6. Review risky candidates
   - Optional AI review can annotate plan entries, but deterministic rewrite logic does not depend on AI.

## Config

Route:

```text
/config
```

Purpose:

Document the CLI configuration surface. type-tune currently uses command options rather than a dedicated config file.

Opening note:

```text
type-tune is configured through CLI options. The generated plan stores the resolved options so apply can validate the plan against the current project state.
```

### Config Page Structure

Sections:

1. Shared scan/plan options
2. `scan` options
3. `plan` options
4. `apply` options
5. `extract` options
6. AI review options
7. Generated and existing types behavior
8. Path alias behavior

### Shared Scan/Plan Options

| Option                      | Default          | Description                                                                                |
| --------------------------- | ---------------- | ------------------------------------------------------------------------------------------ |
| `--project <path>`          | `tsconfig.json`  | Path to the TypeScript project config.                                                     |
| `--min-referrers <number>`  | project default  | Minimum unique referrer files required for scan and plan candidates.                       |
| `--placement-style <style>` | project default  | Destination strategy. Supported: `collocation`, `repo-types`.                              |
| `--include <kinds>`         | `type,interface` | Declaration kinds to include. Supports `type` and `interface`.                             |
| `--ignore-file <path>`      | none             | Ignore entries file. Entries can use candidate id, stable key, or `sourcePath#SymbolName`. |
| `--include-existing-types`  | `false`          | Include declarations already under a `types` directory.                                    |
| `--include-generated`       | `false`          | Include generated source files in analysis.                                                |

### Scan Options

| Option              | Default         | Description                                                  |
| ------------------- | --------------- | ------------------------------------------------------------ |
| `--format <format>` | project default | Output format. Supported: `text`, `json`.                    |
| `--verbose`         | `false`         | Include referrer and import strategy details in text output. |
| `--explain`         | `false`         | Explain exported declarations omitted from scan results.     |

Example:

```bash
type-tune scan \
  --project tsconfig.json \
  --format text \
  --verbose \
  --explain
```

### Plan Options

| Option                      | Default                | Description                                              |
| --------------------------- | ---------------------- | -------------------------------------------------------- |
| `--out <path>`              | `.type-tune/plan.json` | Path to write structured plan JSON.                      |
| `--markdown <path>`         | `.type-tune/plan.md`   | Path to write human-reviewable Markdown.                 |
| `--default-status <status>` | `apply`                | Initial plan entry status. Supported: `apply`, `ignore`. |

Example:

```bash
type-tune plan \
  --project tsconfig.json \
  --out .type-tune/plan.json \
  --markdown .type-tune/plan.md
```

### Apply Options

| Option              | Default  | Description                                                   |
| ------------------- | -------- | ------------------------------------------------------------- |
| `--plan <path>`     | required | Path to `plan.json`.                                          |
| `--markdown <path>` | none     | Path to reviewed `plan.md`; applies edited statuses.          |
| `--dry-run`         | `false`  | Preview changes, run validation, then roll back file changes. |

Example:

```bash
type-tune apply \
  --plan .type-tune/plan.json \
  --markdown .type-tune/plan.md \
  --dry-run
```

### Extract Options

| Option                      | Default         | Description                                         |
| --------------------------- | --------------- | --------------------------------------------------- |
| `<symbol>`                  | required        | Symbol name to extract.                             |
| `--from <path>`             | required        | Source file containing the symbol.                  |
| `--to <path>`               | derived         | Explicit destination file path.                     |
| `--project <path>`          | `tsconfig.json` | Path to the TypeScript project config.              |
| `--placement-style <style>` | project default | Destination strategy when `--to` is omitted.        |
| `--dry-run`                 | `false`         | Preview changes without keeping rewrites.           |
| `--include-generated`       | `false`         | Include generated source files in project analysis. |

Example:

```bash
type-tune extract OrderSummary \
  --from src/features/order/schema.ts \
  --to src/features/order/types/order.ts \
  --dry-run
```

### AI Review Options

AI review is optional and only annotates the plan. It should be presented as a review aid, not as part of the deterministic rewrite path.

| Option                            | Default          | Description                                                                       |
| --------------------------------- | ---------------- | --------------------------------------------------------------------------------- |
| `--ai-review`                     | `false`          | Prepare AI review for generated plan.                                             |
| `--ai-dry-run`                    | `false`          | Write the AI review request without calling a provider.                           |
| `--ai-provider <provider>`        | `mock`           | Provider name. Planned docs should list supported providers from the current CLI. |
| `--ai-context <mode>`             | `snippets`       | Context mode. Supported: `summary`, `snippets`, `full`.                           |
| `--ai-request-out <path>`         | none             | Path to write AI review request JSON.                                             |
| `--ai-response-file <path>`       | none             | Mock provider response JSON.                                                      |
| `--ai-endpoint <url>`             | provider default | Chat completions endpoint.                                                        |
| `--ai-model <model>`              | none             | AI model name.                                                                    |
| `--ai-api-key-env <name>`         | provider default | Environment variable name containing API key.                                     |
| `--ai-timeout-ms <number>`        | `60000`          | AI request timeout in milliseconds.                                               |
| `--ai-on-error <mode>`            | `fail`           | Error handling. Supported: `fail`, `skip`, `mark-needs-review`.                   |
| `--ai-cache <path>`               | none             | AI review cache JSON path.                                                        |
| `--ai-max-candidates <number>`    | none             | Maximum candidates sent to AI review.                                             |
| `--ai-max-context-bytes <number>` | none             | Maximum source context bytes sent to AI review.                                   |

Example:

```bash
type-tune plan \
  --project tsconfig.json \
  --ai-review \
  --ai-provider openai \
  --ai-model gpt-4.1-mini \
  --ai-cache .type-tune/ai-review-cache.json
```

## Content Data Design

Keep docs content data centralized so page components stay small.

```text
docs/src/shared/config/install-commands.ts
docs/src/shared/config/navigation.ts
docs/src/shared/config/cli-options.ts
```

Example data shape:

```ts
export type PackageManager = "pnpm" | "yarn" | "bun" | "npm";

export type InstallCommand = {
  manager: PackageManager;
  command: string;
};

export type CliOption = {
  command: "shared" | "scan" | "plan" | "apply" | "extract" | "ai-review";
  name: string;
  value?: string;
  defaultValue?: string;
  description: string;
};
```

## Visual Direction

Keep the UI quiet and utility-focused.

Guidelines:

- Use compact sections and code examples over decorative hero blocks.
- Prefer tables for config reference.
- Use code tabs for install commands.
- Use clear callouts for deterministic behavior, dry-run safety, and generated file defaults.
- Avoid heavy marketing copy.
- Keep cards only for repeated items such as usecases or link groups.

Suggested palette:

- Background: neutral white or near-white.
- Text: dark neutral.
- Accent: one restrained brand color for links and active nav.
- Code blocks: high-contrast but not overly dark.

## Implementation Phases

### Phase 1: Static Site Skeleton

- Add `docs/` Astro app.
- Add header, shell, sidebar, and basic typography.
- Create Home, Usage, and Config pages.
- Add GitHub link and install tabs.

### Phase 2: Content Completeness

- Fill usage guide from README content.
- Add config option tables.
- Add examples for scan, plan, apply, extract, and AI review.
- Add footer links to changelog, contributing, security, and npm.

### Phase 3: Polish

- Add copy buttons for commands.
- Add active sidebar state.
- Add responsive checks for mobile and desktop.
- Add search only if the docs grow beyond the first three pages.

## Acceptance Criteria

- `docs/` is a standalone Astro workspace package.
- Home page includes tagline, install commands for pnpm/yarn/bun/npm, short usage, links, and GitHub.
- Usage page explains why type-tune exists, how to use it, and practical usecases.
- Config page lists current CLI options with defaults and descriptions.
- Layout follows the clarity of `../is-kit/docs`: fixed header, optional sidebar, reusable code blocks, centralized constants.
- Site builds with `pnpm --filter ./docs build`.
