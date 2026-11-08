# Type-only barrel re-export order

The rewrite can move a standalone, single-specifier `export type` after later value re-exports;
`move_type_only_reexport_after_values` applies this narrow case. Preserve its statement and module
specifier conventions when changing this behavior.

More importantly, a barrel statement containing moved and unmoved symbols must not redirect an
unmoved symbol. Preserve names and aliases, avoid overlapping edits when candidates share a barrel,
and retain the original route when a split cannot be made safely and the source compatibility
re-export keeps it valid. Test mixed, renamed, wildcard, and multi-candidate exports with compilation.

Relevant code: `crates/type-tune-core/src/rewrite/barrel.rs`,
`crates/type-tune-core/src/rewrite/reexports.rs`, and `crates/type-tune-core/src/rewrite/tests.rs`.
