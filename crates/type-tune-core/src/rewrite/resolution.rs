//! Resolution of barrel and inline type references during rewrites.

use crate::{
    model::{Candidate, ImportStrategy},
    module_path::{
        PackageAliasScopes, resolve_relative_module_with_options,
        resolve_scoped_path_alias_with_base_url_and_module_suffixes,
    },
    parser::ParsedSource,
};
use std::collections::{BTreeMap, HashSet};

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_known_module_path(
    referrer_path: &str,
    module_specifier: &str,
    known_paths: &HashSet<String>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
) -> Option<String> {
    resolve_relative_module_with_options(
        referrer_path,
        module_specifier,
        known_paths,
        root_dirs,
        module_suffixes,
    )
    .or_else(|| {
        resolve_scoped_path_alias_with_base_url_and_module_suffixes(
            referrer_path,
            module_specifier,
            base_url,
            path_aliases,
            package_aliases,
            known_paths,
            module_suffixes,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn find_named_barrel_reexport_to_candidate(
    barrel_path: &str,
    imported_name: &str,
    candidate: &Candidate,
    parsed_sources: &BTreeMap<String, ParsedSource>,
    known_paths: &HashSet<String>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
    visited: &mut HashSet<String>,
) -> Option<(String, crate::parser::ParsedReExport)> {
    if !visited.insert(barrel_path.into()) {
        return None;
    }
    let barrel = parsed_sources.get(barrel_path)?;
    for export in &barrel.re_exports {
        if export.is_wildcard || export.exported_name != imported_name {
            continue;
        }
        let source_path = resolve_known_module_path(
            barrel_path,
            &export.module_specifier,
            known_paths,
            base_url,
            path_aliases,
            package_aliases,
            root_dirs,
            module_suffixes,
        )?;
        if source_path == candidate.source_path && export.source_name == candidate.symbol_name {
            return Some((barrel_path.into(), export.clone()));
        }
        if let Some(found) = find_named_barrel_reexport_to_candidate(
            &source_path,
            &export.source_name,
            candidate,
            parsed_sources,
            known_paths,
            base_url,
            path_aliases,
            package_aliases,
            root_dirs,
            module_suffixes,
            visited,
        ) {
            return Some(found);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn inline_type_import_resolves_candidate(
    referrer_path: &str,
    import: &crate::parser::ParsedInlineTypeImport,
    candidate: &Candidate,
    parsed_sources: &BTreeMap<String, ParsedSource>,
    known_paths: &HashSet<String>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
) -> bool {
    let Some(module_path) = resolve_known_module_path(
        referrer_path,
        &import.module_specifier,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
    ) else {
        return false;
    };
    if module_path == candidate.source_path && import.imported_name == candidate.symbol_name {
        return true;
    }
    if find_named_barrel_reexport_to_candidate(
        &module_path,
        &import.imported_name,
        candidate,
        parsed_sources,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        &mut HashSet::new(),
    )
    .is_some()
    {
        return true;
    }
    if barrel_has_wildcard_path_to_candidate(
        &module_path,
        candidate,
        parsed_sources,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        &mut HashSet::new(),
    ) {
        return true;
    }
    let Some(barrel) = parsed_sources.get(&module_path) else {
        return false;
    };
    barrel.re_exports.iter().any(|export| {
        (export.is_wildcard || export.exported_name == import.imported_name)
            && (export.is_wildcard || export.source_name == candidate.symbol_name)
            && resolve_known_module_path(
                &module_path,
                &export.module_specifier,
                known_paths,
                base_url,
                path_aliases,
                package_aliases,
                root_dirs,
                module_suffixes,
            )
            .as_deref()
                == Some(candidate.source_path.as_str())
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn inline_type_reference_uses_source_compatibility(
    referrer_path: &str,
    import: &crate::parser::ParsedInlineTypeImport,
    candidate: &Candidate,
    parsed_sources: &BTreeMap<String, ParsedSource>,
    known_paths: &HashSet<String>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
) -> bool {
    let Some(module_path) = resolve_known_module_path(
        referrer_path,
        &import.module_specifier,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
    ) else {
        return false;
    };
    if module_path == candidate.source_path && import.imported_name == candidate.symbol_name {
        return true;
    }
    barrel_has_wildcard_path_to_candidate(
        &module_path,
        candidate,
        parsed_sources,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        &mut HashSet::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn barrel_has_wildcard_path_to_candidate(
    barrel_path: &str,
    candidate: &Candidate,
    parsed_sources: &BTreeMap<String, ParsedSource>,
    known_paths: &HashSet<String>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
    visited: &mut HashSet<String>,
) -> bool {
    if !visited.insert(barrel_path.into()) {
        return false;
    }
    parsed_sources.get(barrel_path).is_some_and(|barrel| {
        barrel.re_exports.iter().any(|export| {
            let Some(source_path) = resolve_known_module_path(
                barrel_path,
                &export.module_specifier,
                known_paths,
                base_url,
                path_aliases,
                package_aliases,
                root_dirs,
                module_suffixes,
            ) else {
                return false;
            };
            (export.is_wildcard && source_path == candidate.source_path)
                || barrel_has_wildcard_path_to_candidate(
                    &source_path,
                    candidate,
                    parsed_sources,
                    known_paths,
                    base_url,
                    path_aliases,
                    package_aliases,
                    root_dirs,
                    module_suffixes,
                    visited,
                )
        })
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn has_inline_type_referrers(
    source_candidates: &[&Candidate],
    parsed_sources: &BTreeMap<String, ParsedSource>,
    known_paths: &HashSet<String>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
) -> bool {
    source_candidates.iter().any(|candidate| {
        candidate.referrer_strategies.iter().any(|referrer| {
            let Some(parsed_referrer) = parsed_sources.get(&referrer.referrer_path) else {
                return false;
            };
            parsed_referrer
                .import_analysis
                .inline_type_imports
                .iter()
                .any(|import| {
                    inline_type_import_resolves_candidate(
                        &referrer.referrer_path,
                        import,
                        candidate,
                        parsed_sources,
                        known_paths,
                        base_url,
                        path_aliases,
                        package_aliases,
                        root_dirs,
                        module_suffixes,
                    )
                })
        })
    })
}

/// Resolve an import according to the candidate's referrer strategy.
pub(super) fn resolve_referrer_import(
    context: super::referrer_edits::ReferrerEditContext<'_>,
    referrer: &crate::model::ReferrerStrategy,
    module_specifier: &str,
) -> Option<String> {
    let resolve_alias = || {
        resolve_scoped_path_alias_with_base_url_and_module_suffixes(
            &referrer.referrer_path,
            module_specifier,
            context.base_url,
            context.path_aliases,
            context.package_aliases,
            context.known_paths,
            context.module_suffixes,
        )
    };
    match &referrer.strategy {
        ImportStrategy::Relative { .. } | ImportStrategy::Alias { .. } => {
            resolve_relative_module_with_options(
                &referrer.referrer_path,
                module_specifier,
                context.known_paths,
                context.root_dirs,
                context.module_suffixes,
            )
            .or_else(resolve_alias)
        }
        ImportStrategy::Barrel { .. } => None,
    }
}
