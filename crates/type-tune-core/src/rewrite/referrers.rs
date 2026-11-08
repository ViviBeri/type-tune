//! Planning merged imports in referrer modules.

use super::format::{
    format_named_from_statement, import_has_attached_comments, quoted_module_specifier,
    statement_semicolon,
};
use crate::{
    Result, TypeTuneError,
    model::{Candidate, ImportStrategy},
    module_path::{
        PackageAliasScopes, configured_alias_pattern, resolve_relative_module_with_options,
        resolve_scoped_path_alias_with_base_url_and_module_suffixes,
    },
    module_specifier::is_relative_module_specifier,
    parser::ParsedSource,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub(super) struct MergedReferrerImports {
    pub(super) edits: Vec<(String, usize, usize, String)>,
    /// Statement ranges whose imports are already rewritten or removed.
    pub(super) ranges: Vec<(String, usize, usize)>,
    /// Mixed imports whose moved names were included in another import.
    pub(super) absorbed_partials: BTreeSet<(String, usize, usize)>,
    /// Extra moved names a mixed import should add to its new import line.
    pub(super) extra_partial_names: BTreeMap<(String, usize, usize), Vec<String>>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn plan_merged_referrer_imports(
    candidates: &[Candidate],
    candidates_by_source: &BTreeMap<String, Vec<&Candidate>>,
    sources: &BTreeMap<String, String>,
    parsed_sources: &BTreeMap<String, ParsedSource>,
    known_paths: &HashSet<String>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
) -> Result<MergedReferrerImports> {
    let mut candidates_by_referrer_and_destination =
        BTreeMap::<(String, String), BTreeSet<(String, String)>>::new();
    for candidate in candidates {
        for referrer in &candidate.referrer_strategies {
            if candidates_by_source.contains_key(&referrer.referrer_path) {
                continue;
            }
            let module_specifier = match &referrer.strategy {
                ImportStrategy::Alias { module_specifier }
                | ImportStrategy::Relative { module_specifier } => module_specifier,
                ImportStrategy::Barrel { .. } => continue,
            };
            candidates_by_referrer_and_destination
                .entry((referrer.referrer_path.clone(), module_specifier.clone()))
                .or_default()
                .insert((candidate.source_path.clone(), candidate.symbol_name.clone()));
        }
    }

    let mut edits = Vec::new();
    let mut ranges = Vec::new();
    let mut absorbed_partials = BTreeSet::new();
    let mut extra_partial_names = BTreeMap::new();
    for ((referrer_path, destination_module_specifier), moved_symbols) in
        candidates_by_referrer_and_destination
    {
        if moved_symbols.len() < 2 {
            continue;
        }
        let source = sources.get(&referrer_path).ok_or_else(|| {
            TypeTuneError::new(format!("Could not find referrer {referrer_path}."))
        })?;
        let import_analysis = &parsed_sources
            .get(&referrer_path)
            .ok_or_else(|| TypeTuneError::new(format!("Could not find referrer {referrer_path}.")))?
            .import_analysis;
        let resolve_import = |module_specifier: &str| {
            if is_relative_module_specifier(module_specifier) {
                resolve_relative_module_with_options(
                    &referrer_path,
                    module_specifier,
                    known_paths,
                    root_dirs,
                    module_suffixes,
                )
            } else {
                resolve_scoped_path_alias_with_base_url_and_module_suffixes(
                    &referrer_path,
                    module_specifier,
                    base_url,
                    path_aliases,
                    package_aliases,
                    known_paths,
                    module_suffixes,
                )
            }
        };
        let mut statements = BTreeMap::<(usize, usize), Vec<_>>::new();
        for import in &import_analysis.imports {
            let resolved_path = resolve_import(&import.module_specifier);
            if resolved_path.as_deref().is_some_and(|path| {
                moved_symbols.contains(&(path.into(), import.imported_name.clone()))
            }) {
                statements
                    .entry((import.statement_start, import.statement_end))
                    .or_default()
                    .push(import);
            }
        }
        if statements.len() < 2 {
            continue;
        }
        if statements
            .keys()
            .any(|(start, end)| import_has_attached_comments(source, *start, *end))
        {
            continue;
        }
        let import_styles = statements
            .values()
            .flatten()
            .map(|import| {
                if is_relative_module_specifier(&import.module_specifier) {
                    (0, String::new())
                } else if let Some(alias) =
                    configured_alias_pattern(&import.module_specifier, path_aliases)
                {
                    (1, alias)
                } else {
                    (2, import.module_specifier.clone())
                }
            })
            .collect::<HashSet<_>>();
        if import_styles.len() > 1 {
            continue;
        }
        let mut full_imports = Vec::new();
        let mut partial_imports = Vec::new();
        let mut full_ranges = Vec::new();
        let mut partial_ranges = Vec::new();
        for ((start, end), imports) in &statements {
            let all_bindings_move = import_analysis
                .imports
                .iter()
                .filter(|import| import.statement_start == *start && import.statement_end == *end)
                .all(|import| {
                    import.is_type_only
                        && !import.is_default
                        && !import.is_namespace
                        && resolve_import(&import.module_specifier)
                            .as_deref()
                            .is_some_and(|path| {
                                moved_symbols.contains(&(path.into(), import.imported_name.clone()))
                            })
                });
            if all_bindings_move {
                full_ranges.push((*start, *end));
                full_imports.extend(imports.iter().copied());
            } else {
                partial_ranges.push((*start, *end));
                partial_imports.extend(imports.iter().copied());
            }
        }
        if full_ranges.len() + partial_ranges.len() < 2 {
            continue;
        }
        if full_ranges.is_empty() && partial_ranges.len() < 2 {
            continue;
        }
        let render_name = |import: &&crate::parser::ParsedImport| {
            if import.imported_name == import.local_name {
                import.imported_name.clone()
            } else {
                format!("{} as {}", import.imported_name, import.local_name)
            }
        };
        let mut ordered = full_imports;
        ordered.extend(partial_imports.iter().copied());
        ordered.sort_by_key(|import| import.statement_start);
        let bindings = ordered.iter().map(render_name).collect::<Vec<_>>();
        let render_bindings = |quoted_specifier: &str, semicolon: &str| {
            format_named_from_statement(
                "import type",
                &bindings,
                quoted_specifier,
                semicolon,
                source,
            )
        };
        full_ranges.sort_unstable();
        partial_ranges.sort_unstable();
        let contiguous = partial_ranges.is_empty()
            && full_ranges.len() >= 2
            && full_ranges
                .windows(2)
                .all(|ranges| source[ranges[0].1..ranges[1].0].trim().is_empty());
        if contiguous {
            let first_import = import_analysis
                .imports
                .iter()
                .find(|import| import.statement_start == full_ranges[0].0)
                .expect("selected import statement has a binding");
            let semicolon = statement_semicolon(source);
            let last_end = full_ranges.last().expect("ranges are non-empty").1;
            let replacement_line_ending = if source[last_end..].starts_with(['\n', '\r']) {
                ""
            } else {
                "\n"
            };
            edits.push((
                referrer_path.clone(),
                full_ranges[0].0,
                last_end,
                format!(
                    "{}{replacement_line_ending}",
                    render_bindings(
                        &quoted_module_specifier(
                            &source[first_import.module_specifier_start
                                ..first_import.module_specifier_end],
                            &destination_module_specifier,
                        ),
                        semicolon,
                    )
                ),
            ));
            ranges.push((referrer_path, full_ranges[0].0, last_end));
            continue;
        }
        if let Some(&(host_start, host_end)) = full_ranges.first() {
            let first_import = import_analysis
                .imports
                .iter()
                .find(|import| import.statement_start == host_start)
                .expect("selected import statement has a binding");
            let semicolon = statement_semicolon(source);
            edits.push((
                referrer_path.clone(),
                host_start,
                host_end,
                render_bindings(
                    &quoted_module_specifier(
                        &source[first_import.module_specifier_start
                            ..first_import.module_specifier_end],
                        &destination_module_specifier,
                    ),
                    semicolon,
                ),
            ));
            for (start, end) in full_ranges {
                ranges.push((referrer_path.clone(), start, end));
                if start == host_start {
                    continue;
                }
                edits.push((
                    referrer_path.clone(),
                    start,
                    trailing_newline_end(source, end),
                    String::new(),
                ));
            }
            for (start, end) in partial_ranges {
                absorbed_partials.insert((referrer_path.clone(), start, end));
            }
            continue;
        }
        let (host_start, host_end) = partial_ranges[0];
        let extra_names = ordered
            .iter()
            .filter(|import| import.statement_start != host_start)
            .map(render_name)
            .collect::<Vec<_>>();
        extra_partial_names.insert((referrer_path.clone(), host_start, host_end), extra_names);
        for (start, end) in partial_ranges.into_iter().skip(1) {
            absorbed_partials.insert((referrer_path.clone(), start, end));
        }
    }
    Ok(MergedReferrerImports {
        edits,
        ranges,
        absorbed_partials,
        extra_partial_names,
    })
}

fn trailing_newline_end(source: &str, statement_end: usize) -> usize {
    if source[statement_end..].starts_with("\r\n") {
        statement_end + 2
    } else if source[statement_end..].starts_with('\n') {
        statement_end + 1
    } else {
        statement_end
    }
}
