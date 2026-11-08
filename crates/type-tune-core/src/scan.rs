//! Native candidate scanning built on the TypeScript parser.

use std::{collections::BTreeMap, path::Path};

use crate::{
    Result, TypeTuneError,
    model::{Candidate, ImportStrategy, ReferrerStrategy, ScanDiagnostic},
    module_path::PackageAliasScopes,
    parser::{ParsedSource, parse_source},
    source::{discover_typescript_files, read_typescript_source_text},
};

mod barrels;
mod declarations;
mod discovery;
mod filter;
mod placement;
mod referrers;
use declarations::{scan_parsed_declarations, unsupported_dependency_diagnostics};
use discovery::{load_tsconfig_source_files, read_project_source_pair};
pub use filter::{
    ScanFilterOptions, apply_referrer_count_exclusions, filter_candidates_with_diagnostics,
    filter_existing_types, filter_ignored_candidates, filter_kinds, filter_min_referrers,
    filter_noop_destinations,
};
pub use placement::{
    apply_placement, apply_placement_with_aliases, override_candidate_destination,
};
use referrers::scan_parsed_project_files_with_aliases;

fn destination_index_path(strategies: &[ReferrerStrategy]) -> Option<String> {
    strategies
        .iter()
        .find_map(|referrer| match &referrer.strategy {
            ImportStrategy::Barrel {
                barrel_file_path, ..
            } => Some(barrel_file_path.clone()),
            ImportStrategy::Alias { .. } | ImportStrategy::Relative { .. } => None,
        })
}

/// Reads one TypeScript source file and scans its exported declarations.
///
/// # Errors
///
/// Returns an error when the file cannot be read or parsed.
pub fn scan_file(path: &Path, source_path: String) -> Result<Vec<Candidate>> {
    let Some(source) = read_typescript_source_text(path).map_err(|error| {
        TypeTuneError::new(format!("Could not read {}: {error}", path.display()))
    })?
    else {
        return Ok(Vec::new());
    };
    scan_declarations([(source_path, source)])
}

/// Loads and scans every supported source file below a TypeScript project root.
///
/// # Errors
///
/// Returns an error when source discovery or reading fails.
pub fn scan_project_root(project_root: &Path) -> Result<Vec<Candidate>> {
    let files = discover_typescript_files(project_root)
        .map_err(|error| TypeTuneError::new(format!("Could not discover sources: {error}")))?
        .into_iter()
        .filter_map(|path| read_project_source_pair(project_root, &path).transpose())
        .collect::<Result<Vec<_>>>()?;
    scan_project_files(files)
}

/// Loads a tsconfig and scans only the source paths included by it.
///
/// # Errors
///
/// Returns an error when the configuration, discovery, or source reading fails.
pub fn scan_tsconfig(tsconfig_path: &Path) -> Result<Vec<Candidate>> {
    scan_tsconfig_with_generated(tsconfig_path, false)
}

/// Loads a tsconfig and scans its sources, optionally including generated files.
///
/// # Errors
///
/// Returns an error when the configuration, discovery, or source reading fails.
pub fn scan_tsconfig_with_generated(
    tsconfig_path: &Path,
    include_generated: bool,
) -> Result<Vec<Candidate>> {
    let (files, config) = load_tsconfig_source_files(tsconfig_path, include_generated)?;
    scan_project_files_with_aliases_and_package_aliases(
        files,
        config.base_url.as_deref(),
        &config.explicit_paths,
        &config.package_aliases,
        &config.root_dirs,
        &config.module_suffixes,
        config.preserve_path_aliases,
        node_module_specifier_extension(config.package_node_condition),
    )
}

/// Loads a tsconfig for explain output, retaining candidates with unsupported dependencies.
///
/// Callers must remove candidates identified by the returned diagnostics after applying
/// command-level filters, preserving the documented diagnostic precedence.
///
/// # Errors
///
/// Returns an error when the configuration, discovery, source reading, or parsing fails.
pub fn scan_tsconfig_with_generated_and_diagnostics(
    tsconfig_path: &Path,
    include_generated: bool,
) -> Result<(Vec<Candidate>, Vec<ScanDiagnostic>)> {
    let (files, config) = load_tsconfig_source_files(tsconfig_path, include_generated)?;
    let parsed_files = parse_project_files(&files)?;
    let diagnostics = unsupported_dependency_diagnostics(&parsed_files);
    let candidates = scan_parsed_project_files_with_aliases(
        parsed_files,
        config.base_url.as_deref(),
        &config.explicit_paths,
        &config.package_aliases,
        &config.root_dirs,
        &config.module_suffixes,
        true,
        config.preserve_path_aliases,
        node_module_specifier_extension(config.package_node_condition),
    )?;
    Ok((candidates, diagnostics))
}

/// Scans parsed source files for exported declaration candidates.
///
/// Referrer analysis is intentionally added separately; this stage establishes deterministic
/// declaration discovery and stable IDs for the native pipeline.
pub fn scan_declarations(
    files: impl IntoIterator<Item = (String, String)>,
) -> Result<Vec<Candidate>> {
    let parsed_files = files
        .into_iter()
        .map(|(source_path, source)| Ok((source_path, parse_source(&source)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(scan_parsed_declarations(&parsed_files))
}

/// Scans project sources and attaches unique files that import and use each declaration.
///
/// # Errors
///
/// Returns an error when a source file cannot be parsed.
pub fn scan_project_files(files: Vec<(String, String)>) -> Result<Vec<Candidate>> {
    scan_project_files_with_aliases(files, None, &BTreeMap::new())
}

/// Scans project sources with TypeScript path alias resolution.
///
/// # Errors
///
/// Returns an error when a source file cannot be parsed.
pub fn scan_project_files_with_aliases(
    files: Vec<(String, String)>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<Candidate>> {
    scan_project_files_with_aliases_and_root_dirs(files, base_url, path_aliases, &[])
}

/// Scans project sources with TypeScript path-alias and `rootDirs` resolution.
///
/// # Errors
///
/// Returns an error when a source file cannot be parsed.
pub fn scan_project_files_with_aliases_and_root_dirs(
    files: Vec<(String, String)>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    root_dirs: &[String],
) -> Result<Vec<Candidate>> {
    scan_project_files_with_aliases_and_resolution_options(
        files,
        base_url,
        path_aliases,
        root_dirs,
        &[],
    )
}

/// Scans project sources with TypeScript path aliases, `rootDirs`, and `moduleSuffixes`.
///
/// # Errors
///
/// Returns an error when a source file cannot be parsed.
pub fn scan_project_files_with_aliases_and_resolution_options(
    files: Vec<(String, String)>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    root_dirs: &[String],
    module_suffixes: &[String],
) -> Result<Vec<Candidate>> {
    scan_project_files_with_aliases_and_alias_output(
        files,
        base_url,
        path_aliases,
        root_dirs,
        module_suffixes,
        true,
    )
}

fn scan_project_files_with_aliases_and_alias_output(
    files: Vec<(String, String)>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    root_dirs: &[String],
    module_suffixes: &[String],
    preserve_path_aliases: bool,
) -> Result<Vec<Candidate>> {
    scan_project_files_with_aliases_and_package_aliases(
        files,
        base_url,
        path_aliases,
        &PackageAliasScopes::new(),
        root_dirs,
        module_suffixes,
        preserve_path_aliases,
        "",
    )
}

#[allow(clippy::too_many_arguments)]
fn scan_project_files_with_aliases_and_package_aliases(
    files: Vec<(String, String)>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
    preserve_path_aliases: bool,
    default_module_specifier_extension: &str,
) -> Result<Vec<Candidate>> {
    scan_project_files_with_aliases_inner(
        files,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        false,
        preserve_path_aliases,
        default_module_specifier_extension,
    )
}

fn parse_project_files(files: &[(String, String)]) -> Result<Vec<(String, ParsedSource)>> {
    files
        .iter()
        .map(|(path, source)| Ok((path.clone(), parse_source(source)?)))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn scan_project_files_with_aliases_inner(
    files: Vec<(String, String)>,
    base_url: Option<&str>,
    path_aliases: &BTreeMap<String, Vec<String>>,
    package_aliases: &PackageAliasScopes,
    root_dirs: &[String],
    module_suffixes: &[String],
    include_unsupported: bool,
    preserve_path_aliases: bool,
    default_module_specifier_extension: &str,
) -> Result<Vec<Candidate>> {
    let parsed_files = parse_project_files(&files)?;
    scan_parsed_project_files_with_aliases(
        parsed_files,
        base_url,
        path_aliases,
        package_aliases,
        root_dirs,
        module_suffixes,
        include_unsupported,
        preserve_path_aliases,
        default_module_specifier_extension,
    )
}

fn node_module_specifier_extension(package_node_condition: bool) -> &'static str {
    if package_node_condition { ".js" } else { "" }
}

#[cfg(test)]
mod tests;
