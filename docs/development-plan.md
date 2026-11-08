# type-tune development status

## 1. このドキュメントの位置づけ

このドキュメントは、過去の milestone checklist ではなく、現在の実装状態と release に向けた残タスクを追うための status / roadmap である。

- 公開利用手順の正: [README.md](../README.md)
- 要件の背景: [requirements.md](requirements.md)
- 実装設計の詳細: [design.md](design.md)
- AI review の設計: [ai-review-design.md](ai-review-design.md)

古い「Milestone 0 から順に実装する」形式は、実装済み内容と未実装内容が混ざって release 前の判断に使いづらくなったため、この status 形式へ整理した。

## 2. 現在の到達点

MVP の中核である `scan -> plan -> review -> apply` と、単一 symbol 向けの `extract` は実装済みである。

実装済みの CLI:

- `scan`: TypeScript project から抽出候補を検出する
- `plan`: `plan.json` と `plan.md` を生成する
- `review`: `plan.md` を対話的に `[apply]` / `[ignore]` へ更新する
- `apply`: review 済み plan を rewrite / compile validation / rollback 付きで適用する
- `extract`: 指定 symbol を同じ validation path で直接抽出する。scan の referrer threshold
  未満の明示 symbol も対象にでき、`--dry-run` と `--to` をサポートする

実装済みの主要機能:

- `export type` / `export interface` の deterministic scan
- candidate ID と declaration key の stable hash 生成
- generated file と既存 `types` directory の default 除外
- `--include-generated` による generated file の明示 inclusion
- `--include-existing-types` による既存 `types` directory / `types.ts` の明示 inclusion
- `--min-referrers` による候補絞り込み
- `--referrer-count-exclude` による test / story / mock referrer の eligibility count 除外
- `scan --explain` による omitted declaration diagnostics
- `plan --default-status apply|ignore` による初期 review status の制御
- CLI default `--min-referrers 2`
- ignore file による候補除外
- `collocation` / `repo-types` placement
- relative / alias / simple barrel import strategy
- plan markdown と apply dry-run での concrete import/export target 表示
- local type dependency の同時移動
- exported local dependency の referrer 更新
- 元 module の type-only compatibility re-export による star barrel / Vue SFC import 維持
- multi-candidate apply
- declaration key dedupe
- destination conflict detection
- stale plan validation
- dry-run compile validation / rollback
- compile validation failure rollback
- AI review dry-run / provider adapters / cache / context byte limit

## 3. 実装済みテスト範囲

現在のテストは、MVP の主要ケースを Rust unit / command / package smoke test でカバーしている。

代表的な coverage:

- deterministic scan output
- candidate ID / declaration key hash
- generated file exclusion
- explicit generated file inclusion
- scan explain diagnostics for omitted exported declarations
- default plan status handling
- plan/dry-run import target rendering
- existing `types` directory exclusion / inclusion
- unsupported declaration kind exclusion
- default export / namespace merge / declaration merge exclusion
- value symbol / runtime import dependency exclusion
- recursive local dependency collection
- relative / alias / barrel strategy
- plan JSON validation
- markdown review merge
- interactive review key handling
- multi-candidate apply
- shared local dependency dedupe
- type import copy
- star barrel と Vue SFC の未解析 import を維持する source compatibility re-export
- stale plan reject
- declaration conflict reject
- compile validation rollback
- dry-run rollback
- CLI smoke flow
- AI review request / merge / provider adapter behavior
- native binary package の staged checksum verification
- packed root npm package から platform Rust binary を起動する launcher smoke test
- checked-in compatibility fixtures that run the release binary through scan / plan / dry-run / apply
  for alias, barrel, and framework-shaped TypeScript layouts

Local verification command:

```bash
pnpm run check
```

Release packaging smoke:

```bash
pnpm package:check
pnpm publish:check
```

`publish:check` should pass before publishing a release.

## 4. Release blocker

Release 前に必ず片付ける項目:

- npm package name `type-tune` を確保して publish できる状態にする
- `pnpm publish:check` を release 直前に通す
- GitHub Actions secret `NPM_TOKEN` と Environment `npm` を設定する
- npm pack の同梱ファイルを再確認する

GitHub repository は公開済みの [necoz-oss/type-tune](https://github.com/necoz-oss/type-tune) を正とする。
`package.json`、docs site の GitHub link、release workflow はこの repository identity を前提にする。

現在の CI は Rust format / Clippy / test / release build を実行し、`v*` tag または手動実行で
`pnpm publish:check` を実行する。package smoke は `README.md`, `LICENSE`, `package.json`,
`bin/type-tune.mjs` と platform package metadata の同梱を確認する。各 native target は staged
binary の `--help` を実行し、実行可能な host target では packed root npm package から launcher
smoke test も実行する。Linux musl package は GNU runner で build/package validation を行うが、その
runner では musl package を選択できないため root launcher smoke test は対象外である。version bump
は手動 workflow から release PR を作る。
CHANGELOG 更新は `necoz-oss/changelog-bot` を使う Changelog workflow から別 PR として作る。
npm publish は `v*` tag または手動 workflow から実行し、`npm publish --provenance` を使う。

## 5. 公開前に見直すべき品質項目

Release blocker ではないが、公開前に見直す価値が高い項目:

- README と docs の command examples が `--help` の現行 option と一致しているか確認する
- docs の example が現行 `plan.md` status 記法 `[apply]` / `[ignore]` を使っているか確認する
- error message が初見ユーザーに十分 actionable か確認する
- large project での scan / plan 時間を一度測る
- `apply --dry-run` 後の working tree clean を外部 fixture で継続確認する

## 6. v1 以降の候補

公開後の拡張候補:

- config file support
- inline ignore pragma
- richer monorepo discovery
- enum support
- more advanced barrel optimization
- stronger path alias selection
- performance profiling and cache
- machine-readable apply summary
- release workflow automation

これらは現時点の MVP release blocker ではない。`type-tune` の v1 は、対象を `export type` / `export interface` に絞り、reviewable plan と rollback 付き rewrite を安定して提供することを優先する。
