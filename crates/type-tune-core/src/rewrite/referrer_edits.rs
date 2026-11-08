//! Plan edits to imports and re-exports in referrer modules.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{
    barrel::{plan_aliased_barrel_edits, plan_barrel_edits},
    format::{
        earlier_sorted_import_offset, later_import_insert_offset, quoted_module_specifier,
        trailing_import_comment_end,
    },
    mixed_imports::plan_mixed_import_edits,
    resolution::{inline_type_reference_uses_source_compatibility, resolve_referrer_import},
};
use crate::{
    Result, TypeTuneError,
    edit::TextEdit,
    model::{Candidate, ImportStrategy},
    module_path::{
        PackageAliasScopes, is_configured_path_alias,
        resolve_alias_module_specifier_with_known_paths,
    },
    module_specifier::{
        is_relative_module_specifier, preserved_module_specifier_extension,
        relative_module_specifier_with_extension,
    },
    parser::ParsedSource,
};

#[derive(Clone, Copy)]
pub(super) struct ReferrerEditContext<'a> {
    pub(super) candidates: &'a [Candidate],
    pub(super) candidates_by_source: &'a BTreeMap<String, Vec<&'a Candidate>>,
    pub(super) sources: &'a BTreeMap<String, String>,
    pub(super) parsed_sources: &'a BTreeMap<String, ParsedSource>,
    pub(super) known_paths: &'a HashSet<String>,
    pub(super) base_url: Option<&'a str>,
    pub(super) path_aliases: &'a BTreeMap<String, Vec<String>>,
    pub(super) package_aliases: &'a PackageAliasScopes,
    pub(super) root_dirs: &'a [String],
    pub(super) module_suffixes: &'a [String],
    pub(super) module_specifier_extension: &'a str,
    pub(super) merged_referrer_import_ranges: &'a [(String, usize, usize)],
    pub(super) absorbed_partials: &'a BTreeSet<(String, usize, usize)>,
    pub(super) extra_partial_names: &'a BTreeMap<(String, usize, usize), Vec<String>>,
}

pub(super) fn plan_referrer_edits(
    context: ReferrerEditContext<'_>,
    edits_by_path: &mut BTreeMap<String, Vec<TextEdit>>,
) -> Result<()> {
    let ReferrerEditContext {
        candidates,
        candidates_by_source,
        sources,
        parsed_sources,
        known_paths,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        module_specifier_extension: _,
        merged_referrer_import_ranges,
        absorbed_partials: _,
        extra_partial_names: _,
    } = context;
    for candidate in candidates {
        let moved_symbols = candidates_by_source
            .get(&candidate.source_path)
            .map(|candidates| {
                candidates
                    .iter()
                    .map(|candidate| candidate.symbol_name.clone())
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        for referrer in &candidate.referrer_strategies {
            if candidates_by_source.contains_key(&referrer.referrer_path) {
                continue;
            }
            if let ImportStrategy::Barrel {
                barrel_file_path,
                export_module_specifier,
            } = &referrer.strategy
            {
                let edits = plan_barrel_edits(
                    context,
                    candidate,
                    &moved_symbols,
                    barrel_file_path,
                    export_module_specifier,
                )?;
                edits_by_path
                    .entry(barrel_file_path.clone())
                    .or_default()
                    .extend(edits);
                continue;
            }
            let source = sources.get(&referrer.referrer_path).ok_or_else(|| {
                TypeTuneError::new(format!(
                    "Could not find referrer {}.",
                    referrer.referrer_path
                ))
            })?;
            let mut edits = Vec::new();
            let mut preserved_namespace_import = false;
            let mut merged_referrer_import = false;
            let import_analysis = &parsed_sources
                .get(&referrer.referrer_path)
                .ok_or_else(|| {
                    TypeTuneError::new(format!(
                        "Could not find referrer {}.",
                        referrer.referrer_path
                    ))
                })?
                .import_analysis;
            let mixed_import_edits =
                plan_mixed_import_edits(context, candidate, referrer, source, import_analysis);
            let mixed_ranges = mixed_import_edits
                .iter()
                .filter(|edit| edit.start < edit.end)
                .map(|edit| (edit.start, edit.end))
                .collect::<Vec<_>>();
            let has_mixed_edits = !mixed_import_edits.is_empty();
            if !mixed_import_edits.is_empty() {
                edits_by_path
                    .entry(referrer.referrer_path.clone())
                    .or_default()
                    .extend(mixed_import_edits);
            }
            for import in &import_analysis.imports {
                if mixed_ranges.contains(&(import.statement_start, import.statement_end)) {
                    continue;
                }
                if merged_referrer_import_ranges
                    .iter()
                    .any(|(path, start, end)| {
                        path == &referrer.referrer_path
                            && *start <= import.statement_start
                            && import.statement_end <= *end
                    })
                {
                    merged_referrer_import = true;
                    continue;
                }
                let imported_candidate = import.imported_name == candidate.symbol_name
                    || (import.is_namespace
                        && import_analysis
                            .usage
                            .namespace_members
                            .get(&import.local_name)
                            .is_some_and(|members| members.contains(&candidate.symbol_name)));
                let resolved_path =
                    resolve_referrer_import(context, referrer, &import.module_specifier);
                if imported_candidate
                    && resolved_path.as_deref() == Some(candidate.source_path.as_str())
                {
                    // Keep namespace imports pointed at the original module. The
                    // compatibility re-export added to that module preserves the
                    // namespace shape, whereas the TypeScript implementation only
                    // relocates named imports.
                    if import.is_namespace {
                        preserved_namespace_import = true;
                        continue;
                    }
                    let module_specifier = destination_specifier_for_import(
                        context,
                        candidate,
                        &referrer.referrer_path,
                        &import.module_specifier,
                    );
                    let moves_entire_inline_type_only_import = import_analysis
                        .imports
                        .iter()
                        .filter(|binding| {
                            binding.statement_start == import.statement_start
                                && binding.statement_end == import.statement_end
                        })
                        .all(|binding| {
                            binding.is_type_only
                                && !binding.is_default
                                && !binding.is_namespace
                                && binding.imported_name == candidate.symbol_name
                        });
                    if moves_entire_inline_type_only_import {
                        let moved = rewrite_inline_type_import_as_type_only(
                            source,
                            import,
                            &module_specifier,
                        );
                        if !moved.is_empty() {
                            edits.extend(moved);
                            continue;
                        }
                    }
                    let statement = &source[import.statement_start..import.statement_end];
                    let quoted = quoted_module_specifier(
                        &source[import.module_specifier_start..import.module_specifier_end],
                        &module_specifier,
                    );
                    let mut replacement = statement.to_owned();
                    replacement.replace_range(
                        import.module_specifier_start - import.statement_start
                            ..import.module_specifier_end - import.statement_start,
                        &quoted,
                    );
                    let single_binding = import_analysis
                        .imports
                        .iter()
                        .filter(|binding| {
                            binding.statement_start == import.statement_start
                                && binding.statement_end == import.statement_end
                        })
                        .count()
                        == 1;
                    if single_binding
                        && statement.trim_start().starts_with("import type ")
                        && let Some(at) = earlier_sorted_import_offset(
                            source,
                            import.statement_start,
                            import.statement_end,
                            &replacement,
                        )
                    {
                        let after = &source[import.statement_end..];
                        let newline_len = if after.starts_with("\r\n") {
                            2
                        } else if after.starts_with('\n') {
                            1
                        } else {
                            0
                        };
                        if newline_len > 0 {
                            edits.push(TextEdit {
                                start: at,
                                end: at,
                                replacement: format!("{replacement}\n"),
                            });
                            edits.push(TextEdit {
                                start: import.statement_start,
                                end: import.statement_end + newline_len,
                                replacement: String::new(),
                            });
                            continue;
                        }
                    }
                    edits.push(TextEdit {
                        start: import.module_specifier_start,
                        end: import.module_specifier_end,
                        replacement: quoted_module_specifier(
                            &source[import.module_specifier_start..import.module_specifier_end],
                            &module_specifier,
                        ),
                    });
                }
            }
            let retains_inline_type_reference =
                import_analysis.inline_type_imports.iter().any(|import| {
                    inline_type_reference_uses_source_compatibility(
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
                });
            if retains_inline_type_reference && edits.is_empty() {
                continue;
            }
            if edits.is_empty() {
                if has_mixed_edits {
                    continue;
                }
                let (aliased_barrel_edits, barrel_preserves_namespace) = plan_aliased_barrel_edits(
                    context,
                    candidate,
                    &referrer.referrer_path,
                    &moved_symbols,
                    import_analysis,
                );
                preserved_namespace_import |= barrel_preserves_namespace;
                if preserved_namespace_import || merged_referrer_import {
                    continue;
                }
                if !aliased_barrel_edits.is_empty() {
                    for (barrel_path, edit) in aliased_barrel_edits {
                        edits_by_path.entry(barrel_path).or_default().push(edit);
                    }
                    continue;
                }
                return Err(TypeTuneError::new(format!(
                    "Could not find referrer import for {} in {}.",
                    candidate.symbol_name, referrer.referrer_path
                )));
            }
            edits_by_path
                .entry(referrer.referrer_path.clone())
                .or_default()
                .extend(edits);
        }
    }
    Ok(())
}

pub(super) fn destination_specifier_for_import(
    context: ReferrerEditContext<'_>,
    candidate: &Candidate,
    referrer_path: &str,
    original_specifier: &str,
) -> String {
    let extension = preserved_module_specifier_extension(original_specifier)
        .unwrap_or(context.module_specifier_extension);
    if !is_relative_module_specifier(original_specifier)
        && is_configured_path_alias(original_specifier, context.path_aliases)
        && let Some(alias) = resolve_alias_module_specifier_with_known_paths(
            &candidate.destination_path,
            context.base_url,
            context.path_aliases,
            Some(original_specifier),
            Some(context.known_paths),
            context.module_suffixes,
            extension,
        )
    {
        return format!("{alias}{extension}");
    }
    relative_module_specifier_with_extension(referrer_path, &candidate.destination_path, extension)
}

fn rewrite_inline_type_import_as_type_only(
    source: &str,
    import: &crate::parser::ParsedImport,
    module_specifier: &str,
) -> Vec<TextEdit> {
    let statement = &source[import.statement_start..import.statement_end];
    let mut replacement = statement.replacen("import { type ", "import type { ", 1);
    if replacement == statement {
        return Vec::new();
    }
    let module_specifier_start = import.module_specifier_start - import.statement_start;
    let module_specifier_end = import.module_specifier_end - import.statement_start;
    replacement.replace_range(
        module_specifier_start..module_specifier_end,
        &quoted_module_specifier(
            &statement[module_specifier_start..module_specifier_end],
            module_specifier,
        ),
    );
    if trailing_import_comment_end(source, import.statement_end).is_none()
        && let Some(insert_at) =
            later_import_insert_offset(source, import.statement_end, &replacement)
    {
        let mut end = import.statement_end;
        let bytes = source.as_bytes();
        if bytes.get(end) == Some(&b'\r') {
            end += 1;
        }
        if bytes.get(end) == Some(&b'\n') {
            end += 1;
        }
        if insert_at >= end {
            return vec![
                TextEdit {
                    start: import.statement_start,
                    end,
                    replacement: String::new(),
                },
                TextEdit {
                    start: insert_at,
                    end: insert_at,
                    replacement: format!("{replacement}\n"),
                },
            ];
        }
    }
    vec![TextEdit {
        start: import.statement_start,
        end: import.statement_end,
        replacement,
    }]
}
