//! Barrel re-export graph resolution for project scans.

use crate::{
    Result,
    module_path::{
        PackageAliasScopes, resolve_relative_module_with_options,
        resolve_scoped_path_alias_with_base_url_and_module_suffixes,
    },
    parser::ParsedSource,
};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BarrelExport {
    pub(super) barrel_path: String,
    pub(super) source_path: String,
    pub(super) source_name: String,
    pub(super) exported_name: String,
    pub(super) is_wildcard: bool,
    pub(super) is_namespace_wildcard: bool,
    pub(super) module_specifier: String,
}

pub(super) fn resolve_barrel_exports(
    files: &[(String, ParsedSource)],
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
    known_paths: &HashSet<String>,
) -> Result<HashMap<(String, String), Vec<BarrelExport>>> {
    let mut exports = HashMap::<(String, String), Vec<BarrelExport>>::new();
    for (barrel_path, parsed_source) in files {
        for export in &parsed_source.re_exports {
            let Some(source_path) = resolve_relative_module_with_options(
                barrel_path,
                &export.module_specifier,
                known_paths,
                root_dirs,
                module_suffixes,
            )
            .or_else(|| {
                resolve_scoped_path_alias_with_base_url_and_module_suffixes(
                    barrel_path,
                    &export.module_specifier,
                    base_url,
                    path_aliases,
                    package_aliases,
                    known_paths,
                    module_suffixes,
                )
            }) else {
                continue;
            };
            exports
                .entry((barrel_path.clone(), export.exported_name.clone()))
                .or_default()
                .push(BarrelExport {
                    barrel_path: barrel_path.clone(),
                    source_path,
                    source_name: export.source_name.clone(),
                    exported_name: export.exported_name.clone(),
                    is_wildcard: export.is_wildcard,
                    is_namespace_wildcard: export.is_namespace_wildcard,
                    module_specifier: export.module_specifier.clone(),
                });
        }
    }
    loop {
        let snapshot = exports.clone();
        let mut changed = false;
        for ((barrel_path, exported_name), barrel_exports) in &snapshot {
            for barrel_export in barrel_exports {
                let nested_exports = snapshot
                    .get(&(
                        barrel_export.source_path.clone(),
                        barrel_export.source_name.clone(),
                    ))
                    .into_iter()
                    .flatten()
                    .chain(
                        snapshot
                            .get(&(barrel_export.source_path.clone(), "*".into()))
                            .into_iter()
                            .flatten(),
                    );
                for nested_export in nested_exports {
                    let resolved = BarrelExport {
                        barrel_path: barrel_path.clone(),
                        source_path: nested_export.source_path.clone(),
                        source_name: if nested_export.is_wildcard {
                            barrel_export.source_name.clone()
                        } else {
                            nested_export.source_name.clone()
                        },
                        exported_name: exported_name.clone(),
                        is_wildcard: barrel_export.is_wildcard,
                        is_namespace_wildcard: barrel_export.is_namespace_wildcard,
                        module_specifier: barrel_export.module_specifier.clone(),
                    };
                    let resolved_exports = exports
                        .entry((barrel_path.clone(), exported_name.clone()))
                        .or_default();
                    if !resolved_exports.contains(&resolved) {
                        resolved_exports.push(resolved);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(exports)
}
