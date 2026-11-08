//! Plan edits for referrers using a barrel import strategy.

use std::collections::{BTreeSet, HashSet};

use crate::{Result, TypeTuneError, edit::TextEdit, model::Candidate};

use crate::{
    module_path::{
        resolve_relative_module_with_options,
        resolve_scoped_path_alias_with_base_url_and_module_suffixes,
    },
    module_specifier::{
        preserved_module_specifier_extension, relative_module_specifier_with_extension,
    },
    parser::ParsedImportAnalysis,
};

use super::{
    format::{import_has_attached_comments, quoted_module_specifier, statement_semicolon},
    reexports::{
        inline_type_reexport_edits, move_type_only_reexport_after_values,
        rewrite_reexport_module_specifier, split_mixed_inline_type_reexport,
    },
    referrer_edits::ReferrerEditContext,
    resolution::{find_named_barrel_reexport_to_candidate, resolve_known_module_path},
};

pub(super) fn plan_barrel_edits(
    context: ReferrerEditContext<'_>,
    candidate: &Candidate,
    moved_symbols: &BTreeSet<String>,
    barrel_file_path: &str,
    export_module_specifier: &str,
) -> Result<Vec<TextEdit>> {
    let ReferrerEditContext {
        sources,
        parsed_sources,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        ..
    } = context;
    let barrel_source = sources
        .get(barrel_file_path)
        .ok_or_else(|| TypeTuneError::new(format!("Could not find barrel {barrel_file_path}.")))?;
    let barrel_exports = parsed_sources
        .get(barrel_file_path)
        .ok_or_else(|| TypeTuneError::new(format!("Could not find barrel {barrel_file_path}.")))?;
    let matching_exports = barrel_exports
        .re_exports
        .iter()
        .filter(|export| {
            (export.is_wildcard
                || (export.exported_name == candidate.symbol_name
                    && export.source_name == candidate.symbol_name))
                && resolve_known_module_path(
                    barrel_file_path,
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
        .collect::<Vec<_>>();
    if matching_exports.iter().any(|export| {
        (export.is_inline_type_only
            && import_has_attached_comments(
                barrel_source,
                export.statement_start,
                export.statement_end,
            ))
            || (!export.is_inline_type_only
                && has_unmoved_named_reexports(&barrel_exports.re_exports, export, moved_symbols))
            || may_redirect_value_export(context, candidate, moved_symbols, barrel_source, export)
            || (export.is_wildcard
                && !export.is_type_wildcard
                && source_has_remaining_code(context, candidate, moved_symbols))
    }) {
        // The original source retains compatibility re-exports for moved types.
        return Ok(Vec::new());
    }
    let split_edit = matching_exports.iter().find_map(|export| {
        split_mixed_inline_type_reexport(
            &barrel_exports.re_exports,
            export,
            moved_symbols,
            barrel_source,
            export_module_specifier,
        )
    });
    let mut edits = split_edit.map_or_else(Vec::new, |edit| vec![edit]);
    if edits.is_empty() {
        edits = matching_exports
            .iter()
            .flat_map(|export| {
                if export.is_type_wildcard {
                    let semicolon = statement_semicolon(barrel_source);
                    vec![TextEdit {
                        start: export.statement_end,
                        end: export.statement_end,
                        replacement: moved_symbols
                            .iter()
                            .map(|symbol_name| {
                                format!(
                                    "\nexport type {{ {symbol_name} }} from {}{semicolon}",
                                    quoted_module_specifier(
                                        &barrel_source[export.module_specifier_start
                                            ..export.module_specifier_end],
                                        export_module_specifier,
                                    ),
                                )
                            })
                            .collect::<String>(),
                    }]
                } else if let Some(edits) = move_type_only_reexport_after_values(
                    &barrel_exports.re_exports,
                    export,
                    barrel_source,
                    export_module_specifier,
                ) {
                    edits
                } else {
                    vec![rewrite_reexport_module_specifier(
                        barrel_source,
                        export,
                        export_module_specifier,
                    )]
                }
            })
            .collect::<Vec<_>>();
        for export in matching_exports {
            edits.extend(inline_type_reexport_edits(
                &barrel_exports.re_exports,
                export,
                barrel_source,
            ));
        }
    }
    if edits.is_empty() {
        let nested_export = barrel_exports.re_exports.iter().find(|export| {
            export.exported_name == candidate.symbol_name
                && export.source_name == candidate.symbol_name
                && resolve_known_module_path(
                    barrel_file_path,
                    &export.module_specifier,
                    known_paths,
                    base_url,
                    path_aliases,
                    package_aliases,
                    root_dirs,
                    module_suffixes,
                )
                .is_some_and(|path| path != candidate.source_path)
        });
        if let Some(export) = nested_export {
            let semicolon = statement_semicolon(barrel_source);
            edits.push(TextEdit {
                start: export.statement_end,
                end: export.statement_end,
                replacement: format!(
                    "\nexport type {{ {} }} from {}{semicolon}",
                    candidate.symbol_name,
                    quoted_module_specifier("\"\"", export_module_specifier),
                ),
            });
        }
    }
    if edits.is_empty() {
        return Err(TypeTuneError::new(format!(
            "Could not find barrel export for {} in {barrel_file_path}.",
            candidate.symbol_name
        )));
    }
    Ok(edits)
}

pub(super) fn plan_aliased_barrel_edits(
    context: ReferrerEditContext<'_>,
    candidate: &Candidate,
    referrer_path: &str,
    moved_symbols: &BTreeSet<String>,
    import_analysis: &ParsedImportAnalysis,
) -> (Vec<(String, TextEdit)>, bool) {
    let ReferrerEditContext {
        sources,
        parsed_sources,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        module_specifier_extension,
        ..
    } = context;
    let specifier_extension = |specifier: &str| {
        preserved_module_specifier_extension(specifier)
            .unwrap_or(module_specifier_extension)
            .to_owned()
    };
    let mut preserved_namespace_import = false;
    let mut aliased_barrel_edits = Vec::new();
    for import in &import_analysis.inline_type_imports {
        let Some(initial_barrel_path) = resolve_known_module_path(
            referrer_path,
            &import.module_specifier,
            known_paths,
            base_url,
            path_aliases,
            package_aliases,
            root_dirs,
            module_suffixes,
        ) else {
            continue;
        };
        let Some((barrel_path, export)) = find_named_barrel_reexport_to_candidate(
            &initial_barrel_path,
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
        ) else {
            continue;
        };
        if has_unmoved_named_reexports(
            &parsed_sources[&barrel_path].re_exports,
            &export,
            moved_symbols,
        ) || may_redirect_value_export(
            context,
            candidate,
            moved_symbols,
            &sources[&barrel_path],
            &export,
        ) {
            preserved_namespace_import = true;
            continue;
        }
        aliased_barrel_edits.push((
            barrel_path.clone(),
            TextEdit {
                start: export.module_specifier_start,
                end: export.module_specifier_end,
                replacement: quoted_module_specifier(
                    "\"\"",
                    &relative_module_specifier_with_extension(
                        &barrel_path,
                        &candidate.destination_path,
                        &specifier_extension(&export.module_specifier),
                    ),
                ),
            },
        ));
    }
    for import in &import_analysis.imports {
        let Some(barrel_path) = resolve_relative_module_with_options(
            referrer_path,
            &import.module_specifier,
            known_paths,
            root_dirs,
            module_suffixes,
        )
        .or_else(|| {
            resolve_scoped_path_alias_with_base_url_and_module_suffixes(
                referrer_path,
                &import.module_specifier,
                base_url,
                path_aliases,
                package_aliases,
                known_paths,
                module_suffixes,
            )
        }) else {
            continue;
        };
        let Some(barrel) = parsed_sources.get(&barrel_path) else {
            continue;
        };
        let Some(barrel_source) = sources.get(&barrel_path) else {
            continue;
        };
        for export in &barrel.re_exports {
            let export_source_path = resolve_relative_module_with_options(
                &barrel_path,
                &export.module_specifier,
                known_paths,
                root_dirs,
                module_suffixes,
            )
            .or_else(|| {
                resolve_scoped_path_alias_with_base_url_and_module_suffixes(
                    &barrel_path,
                    &export.module_specifier,
                    base_url,
                    path_aliases,
                    package_aliases,
                    known_paths,
                    module_suffixes,
                )
            });
            let is_namespace_wildcard = export.is_namespace_wildcard
                && export.exported_name == import.imported_name
                && export_source_path.as_deref() == Some(candidate.source_path.as_str())
                && import_analysis
                    .usage
                    .namespace_members
                    .get(&import.local_name)
                    .is_some_and(|members| members.contains(&candidate.symbol_name));
            if is_namespace_wildcard {
                // The source compatibility re-export keeps the type namespace
                // available through the barrel.
                preserved_namespace_import = true;
                continue;
            }
            if export.exported_name != import.imported_name
                || export.source_name != candidate.symbol_name
                || export_source_path.as_deref() != Some(candidate.source_path.as_str())
            {
                continue;
            }
            if export.is_inline_type_only
                && import_has_attached_comments(
                    barrel_source,
                    export.statement_start,
                    export.statement_end,
                )
            {
                preserved_namespace_import = true;
                continue;
            }
            if (!export.is_inline_type_only
                && has_unmoved_named_reexports(&barrel.re_exports, export, moved_symbols))
                || may_redirect_value_export(
                    context,
                    candidate,
                    moved_symbols,
                    barrel_source,
                    export,
                )
            {
                preserved_namespace_import = true;
                continue;
            }
            if let Some(edit) = split_mixed_inline_type_reexport(
                &barrel.re_exports,
                export,
                moved_symbols,
                barrel_source,
                &relative_module_specifier_with_extension(
                    &barrel_path,
                    &candidate.destination_path,
                    &specifier_extension(&export.module_specifier),
                ),
            ) {
                aliased_barrel_edits.push((barrel_path.clone(), edit));
                continue;
            }
            if let Some(edits) = move_type_only_reexport_after_values(
                &barrel.re_exports,
                export,
                barrel_source,
                &relative_module_specifier_with_extension(
                    &barrel_path,
                    &candidate.destination_path,
                    &specifier_extension(&export.module_specifier),
                ),
            ) {
                aliased_barrel_edits
                    .extend(edits.into_iter().map(|edit| (barrel_path.clone(), edit)));
                continue;
            }
            aliased_barrel_edits.push((
                barrel_path.clone(),
                rewrite_reexport_module_specifier(
                    barrel_source,
                    export,
                    &relative_module_specifier_with_extension(
                        &barrel_path,
                        &candidate.destination_path,
                        &specifier_extension(&export.module_specifier),
                    ),
                ),
            ));
            for edit in inline_type_reexport_edits(&barrel.re_exports, export, barrel_source) {
                aliased_barrel_edits.push((barrel_path.clone(), edit));
            }
        }
    }
    (aliased_barrel_edits, preserved_namespace_import)
}

fn has_unmoved_named_reexports(
    exports: &[crate::parser::ParsedReExport],
    export: &crate::parser::ParsedReExport,
    moved_symbols: &BTreeSet<String>,
) -> bool {
    !export.is_wildcard
        && exports.iter().any(|other| {
            other.statement_start == export.statement_start
                && other.statement_end == export.statement_end
                && !moved_symbols.contains(&other.source_name)
        })
}

fn may_redirect_value_export(
    context: ReferrerEditContext<'_>,
    candidate: &Candidate,
    moved_symbols: &BTreeSet<String>,
    barrel_source: &str,
    export: &crate::parser::ParsedReExport,
) -> bool {
    !export.is_wildcard
        && !export.is_inline_type_only
        && !barrel_source[export.statement_start..export.statement_end]
            .trim_start()
            .starts_with("export type ")
        && source_has_remaining_code(context, candidate, moved_symbols)
}

fn source_has_remaining_code(
    context: ReferrerEditContext<'_>,
    candidate: &Candidate,
    moved_symbols: &BTreeSet<String>,
) -> bool {
    let source = &context.sources[&candidate.source_path];
    let parsed = &context.parsed_sources[&candidate.source_path];
    let mut remaining = source.to_owned();
    for declaration in parsed.declarations.iter().rev() {
        if moved_symbols.contains(&declaration.symbol_name) {
            remaining.replace_range(declaration.start_byte..declaration.end_byte, "");
        }
    }
    !remaining
        .trim_matches(|c: char| c.is_whitespace() || c == ';')
        .is_empty()
}
