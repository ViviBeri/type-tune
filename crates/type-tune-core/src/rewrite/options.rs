//! Rewrite-planning configuration types.

use std::collections::BTreeMap;

use crate::module_path::PackageAliasScopes;

/// Quote style used for source compatibility re-exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatibilityReexportQuoteStyle {
    /// Match TypeScript `apply` and plan-driven rewrites.
    Apply,
    /// Match TypeScript direct `extract` (for example double quotes on generated modules).
    Extract,
}

/// Module-resolution settings used while planning an extraction rewrite.
#[derive(Clone, Copy)]
pub struct RewriteOptions<'a> {
    /// Optional TypeScript base URL used for explicit path aliases.
    pub base_url: Option<&'a str>,
    /// Explicit TypeScript path aliases.
    pub path_aliases: &'a BTreeMap<String, Vec<String>>,
    /// Package aliases grouped by source package scope.
    pub package_aliases: &'a PackageAliasScopes,
    /// TypeScript root directories used for relative module resolution.
    pub root_dirs: &'a [String],
    /// TypeScript module suffixes applied during module resolution.
    pub module_suffixes: &'a [String],
    /// Extension appended to generated relative module specifiers when required.
    pub default_module_specifier_extension: &'a str,
    /// Quote style for source compatibility re-exports.
    pub compatibility_quote_style: CompatibilityReexportQuoteStyle,
}
