//! Internal data structures for planning type-only imports.

pub(super) fn source_import_cleanup_replacement(
    source: &str,
    parsed_source: &ParsedSource,
    start: usize,
    end: usize,
    moved_type_names: &HashSet<String>,
) -> String {
    let bindings = parsed_source
        .import_analysis
        .imports
        .iter()
        .filter(|import| import.statement_start == start && import.statement_end == end)
        .collect::<Vec<_>>();
    if bindings.is_empty() {
        return String::new();
    }
    let remaining = bindings
        .iter()
        .filter(|binding| {
            if binding.is_namespace {
                return !moved_type_names.contains(&binding.local_name);
            }
            !binding.is_type_only || !moved_type_names.contains(&binding.local_name)
        })
        .collect::<Vec<_>>();
    let original = &source[start..end];
    let statement_type_only = original
        .trim_start()
        .strip_prefix("import")
        .is_some_and(|rest| rest.trim_start().starts_with("type "));
    if remaining.len() == bindings.len()
        || (remaining.len() < bindings.len()
            && (original.contains("/*") || original.contains("//")))
        || (!statement_type_only && remaining.iter().all(|binding| binding.is_type_only))
    {
        // Preserve embedded comments and the runtime semantics of inline type imports, which
        // depend on TypeScript's module emit options even when every specifier is type-only.
        return original.to_string();
    }
    if remaining.is_empty() {
        return String::new();
    }
    let module_specifier = quoted_module_specifier(
        &source[bindings[0].module_specifier_start..bindings[0].module_specifier_end],
        &bindings[0].module_specifier,
    );
    // `import type { type Name }` is a syntax error. Use one type marker: the statement
    // keyword when every remaining binding is type-only, otherwise the inline modifier.
    let use_import_type = remaining.iter().all(|binding| binding.is_type_only);
    let render_binding = |binding: &crate::parser::ParsedImport| {
        let prefix = if binding.is_type_only && !use_import_type && !binding.is_namespace {
            "type "
        } else {
            ""
        };
        if binding.is_default {
            format!("{prefix}{}", binding.local_name)
        } else if binding.is_namespace {
            format!("* as {}", binding.local_name)
        } else if binding.imported_name == binding.local_name {
            format!("{prefix}{}", binding.imported_name)
        } else {
            format!(
                "{prefix}{} as {}",
                binding.imported_name, binding.local_name
            )
        }
    };
    let default_import = remaining.iter().find(|binding| binding.is_default);
    let named_imports = remaining
        .iter()
        .filter(|binding| !binding.is_default && !binding.is_namespace)
        .map(|binding| render_binding(binding))
        .collect::<Vec<_>>();
    let namespace_imports = remaining
        .iter()
        .filter(|binding| binding.is_namespace)
        .map(|binding| render_binding(binding))
        .collect::<Vec<_>>();
    let import_clause = match (
        default_import,
        named_imports.is_empty(),
        namespace_imports.is_empty(),
    ) {
        (Some(default_import), true, true) => render_binding(default_import),
        (Some(default_import), false, true) => {
            format!(
                "{}, {{ {} }}",
                render_binding(default_import),
                named_imports.join(", ")
            )
        }
        (None, false, true) => {
            let type_prefix = if use_import_type { "type " } else { "" };
            let semicolon = statement_semicolon(source);
            format_named_import_clause(
                &named_imports,
                &source[start..end],
                format!("import {type_prefix} from {module_specifier}{semicolon}")
                    .chars()
                    .count(),
            )
        }
        (None, true, false) if namespace_imports.len() == 1 => namespace_imports[0].clone(),
        (None, false, false) => format!(
            "{{ {} }}, {}",
            named_imports.join(", "),
            namespace_imports.join(", ")
        ),
        _ => format!("{{ {} }}", named_imports.join(", ")),
    };
    let type_prefix = if use_import_type { "type " } else { "" };
    let semicolon = statement_semicolon(source);
    let line_ending = if source[end..].starts_with(['\n', '\r']) {
        ""
    } else {
        "\n"
    };
    format!("import {type_prefix}{import_clause} from {module_specifier}{semicolon}{line_ending}")
}

use super::format::{format_named_import_clause, quoted_module_specifier, statement_semicolon};
use std::collections::{BTreeMap, HashSet};

use crate::{
    Result, TypeTuneError,
    module_path::resolve_relative_module,
    module_specifier::{
        is_relative_module_specifier, preserved_module_specifier_extension,
        relative_module_specifier_with_extension,
    },
    parser::ParsedSource,
};

pub(super) fn collect_imported_type_dependencies(
    parsed_source: &ParsedSource,
    moved: &[&crate::parser::ParsedDeclaration],
    source: &str,
    source_path: &str,
    destination_path: &str,
    known_paths: &HashSet<String>,
    moved_destination_paths: &BTreeMap<String, String>,
) -> Result<ImportedTypeDependencies> {
    let local_names = parsed_source
        .declarations
        .iter()
        .map(|declaration| declaration.symbol_name.as_str())
        .collect::<HashSet<_>>();
    let referenced_names = moved
        .iter()
        .flat_map(|declaration| declaration.referenced_type_names.iter())
        .filter(|name| !local_names.contains(name.as_str()))
        .collect::<HashSet<_>>();
    let mut dependencies = ImportedTypeDependencies::default();
    for import in &parsed_source.import_analysis.imports {
        if !referenced_names.contains(&import.local_name) {
            continue;
        }
        if !import.is_type_only {
            return Err(TypeTuneError::new(format!(
                "Native rewrite requires type-only imports for {}.",
                import.local_name
            )));
        }
        let module_specifier = if is_relative_module_specifier(&import.module_specifier) {
            let source_module =
                resolve_relative_module(source_path, &import.module_specifier, known_paths)
                    .ok_or_else(|| {
                        TypeTuneError::new(format!(
                            "Could not resolve imported type module {}.",
                            import.module_specifier
                        ))
                    })?;
            moved_destination_paths.get(&source_module).map_or_else(
                || {
                    relative_module_specifier_with_extension(
                        destination_path,
                        &source_module,
                        preserved_module_specifier_extension(&import.module_specifier)
                            .unwrap_or_default(),
                    )
                },
                |moved_destination_path| {
                    relative_module_specifier_with_extension(
                        destination_path,
                        moved_destination_path,
                        preserved_module_specifier_extension(&import.module_specifier)
                            .unwrap_or_default(),
                    )
                },
            )
        } else {
            import.module_specifier.clone()
        };
        let quote = source[import.module_specifier_start..import.module_specifier_end]
            .chars()
            .next()
            .filter(|character| matches!(character, '\'' | '"'))
            .unwrap_or('\'');
        let imports = dependencies
            .imports
            .entry((module_specifier, quote))
            .or_default();
        add_import_binding(
            imports,
            &import.local_name,
            &import.imported_name,
            import.is_default,
            import.is_namespace,
        );
        dependencies
            .statement_ranges
            .insert((import.statement_start, import.statement_end));
    }
    Ok(dependencies)
}

#[derive(Debug, Default)]
pub(super) struct ImportedTypeDependencies {
    pub(super) imports: BTreeMap<(String, char), TypeImportRequirement>,
    pub(super) statement_ranges: HashSet<(usize, usize)>,
}

#[derive(Debug, Default)]
pub(super) struct TypeImportRequirement {
    pub(super) default_imports: Vec<String>,
    pub(super) namespace_imports: Vec<String>,
    pub(super) named_imports: Vec<String>,
}

pub(super) fn add_import_binding(
    imports: &mut TypeImportRequirement,
    local_name: &str,
    imported_name: &str,
    is_default: bool,
    is_namespace: bool,
) {
    let bindings = if is_default {
        &mut imports.default_imports
    } else if is_namespace {
        &mut imports.namespace_imports
    } else {
        let name = if imported_name == local_name {
            imported_name.to_owned()
        } else {
            format!("{imported_name} as {local_name}")
        };
        if !imports.named_imports.contains(&name) {
            imports.named_imports.push(name);
        }
        return;
    };
    if !bindings.iter().any(|binding| binding == local_name) {
        bindings.push(local_name.to_owned());
    }
}

pub(super) fn render_type_imports(
    imports: &BTreeMap<(String, char), TypeImportRequirement>,
    semicolon: &str,
) -> String {
    let mut rendered = Vec::new();
    for ((path, quote), requirement) in imports {
        let quoted_path = format!("{quote}{path}{quote}");
        for namespace_import in &requirement.namespace_imports {
            rendered.push(format!(
                "import type * as {namespace_import} from {quoted_path}{semicolon}\n"
            ));
        }
        let mut bindings = Vec::new();
        let mut default_imports = requirement.default_imports.iter();
        if let Some(default_import) = default_imports.next() {
            if requirement.named_imports.is_empty() {
                bindings.push(default_import.clone());
            } else {
                bindings.push(format!(
                    "{default_import}, {{ {} }}",
                    requirement.named_imports.join(", ")
                ));
            }
        } else if !requirement.named_imports.is_empty() {
            bindings.push(format!("{{ {} }}", requirement.named_imports.join(", ")));
        }
        bindings.extend(default_imports.cloned());
        if !bindings.is_empty() {
            rendered.push(format!(
                "import type {} from {quoted_path}{semicolon}\n",
                bindings.join(", ")
            ));
        }
    }
    rendered.join("")
}

pub(super) fn merge_type_imports(
    destination: &mut BTreeMap<(String, char), TypeImportRequirement>,
    source: &BTreeMap<(String, char), TypeImportRequirement>,
) {
    for (module, imports) in source {
        let destination = destination.entry(module.clone()).or_default();
        merge_unique_imports(&mut destination.default_imports, &imports.default_imports);
        merge_unique_imports(
            &mut destination.namespace_imports,
            &imports.namespace_imports,
        );
        merge_unique_imports(&mut destination.named_imports, &imports.named_imports);
    }
}

fn merge_unique_imports(destination: &mut Vec<String>, source: &[String]) {
    for import in source {
        if !destination.contains(import) {
            destination.push(import.clone());
        }
    }
}
