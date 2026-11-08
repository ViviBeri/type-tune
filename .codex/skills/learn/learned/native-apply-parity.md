# Compatibility exports and imports

Moving a type can compile while breaking an original module path or changing runtime imports.
When modifying rewrite planning, preserve type-only compatibility re-exports for moved public types
and keep namespace consumers pointing at a valid module. Check both moved and unmoved bindings
before splitting or merging imports; preserve aliases and avoid altering runtime semantics.

Use minimal edits and retain comments and local formatting where safe. Do not infer that any
particular import grouping or formatter output is universally correct. Test multiple moved types,
shared dependencies, mixed imports, aliases, barrels, and NodeNext extensions as applicable.

Relevant code: `crates/type-tune-core/src/rewrite/compatibility.rs`,
`crates/type-tune-core/src/rewrite/referrer_edits.rs`, `crates/type-tune-core/src/rewrite/imports.rs`,
`crates/type-tune-core/src/rewrite/tests.rs`, and `scripts/compatibility-fixtures.mjs`.
