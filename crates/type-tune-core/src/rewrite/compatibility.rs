//! Compatibility exports for types moved from source modules.

pub(super) fn append_trailing_compatibility_exports(
    remaining_source: &str,
    export_block: &str,
) -> String {
    let (imports, rest, _) = split_leading_import_block_with_gap(remaining_source);
    let remaining = remaining_source.trim_end_matches(['\r', '\n']);
    if remaining.is_empty() {
        return export_block.to_string();
    }
    // A file that only keeps imports stays one block. Any remaining statement gets a blank line
    // before the re-export; the newline that already ends that statement is not that blank line.
    let separator = if !imports.is_empty() && rest.trim().is_empty() {
        "\n"
    } else {
        "\n\n"
    };
    format!("{remaining}{separator}{}\n", export_block.trim())
}

/// Places a compatibility import with the leading import block and the re-export at the end.
///
/// This keeps a blank line before both the remaining statements and the trailing re-export.
pub(super) fn place_remaining_source_compatibility(
    remaining: &str,
    import_line: &str,
    export_line: &str,
) -> String {
    let with_import =
        ensure_blank_after_leading_imports(&insert_leading_import(remaining, import_line));
    let (header, body) = split_leading_file_header(&with_import);
    let (_, rest, _) = split_leading_import_block_with_gap(body);
    let prefix = with_import.trim_end_matches(['\r', '\n']);
    if rest.is_empty() && header.is_empty() {
        format!("{prefix}\n{export_line}\n")
    } else {
        format!("{prefix}\n\n{export_line}\n")
    }
}

use super::{
    CompatibilityReexportQuoteStyle,
    format::{
        ensure_blank_after_leading_imports, format_named_from_statement, insert_leading_import,
        split_leading_file_header, split_leading_import_block_with_gap, statement_semicolon,
    },
};
use crate::{
    Result,
    edit::{TextEdit, apply_text_edits},
    model::Candidate,
    module_specifier::relative_module_specifier_with_extension,
    parser::ParsedDeclaration,
    parser::source_has_type_usage,
};
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn source_compatibility_exports(
    source: &str,
    source_path: &str,
    destination_path: &str,
    moved: &[&crate::parser::ParsedDeclaration],
    extension: &str,
    include_import: bool,
    import_names: Option<&str>,
    compatibility_quote_style: CompatibilityReexportQuoteStyle,
    has_external_referrers: bool,
    has_inline_type_referrers: bool,
) -> String {
    let export_names = exported_compatibility_names(moved);
    let (import, export) = compatibility_type_statements(
        source,
        source_path,
        destination_path,
        &export_names,
        if include_import { import_names } else { None },
        extension,
        compatibility_quote_style,
        has_external_referrers,
        has_inline_type_referrers,
    );
    if include_import {
        format!("\n{import}\n{export}\n")
    } else {
        format!("{export}\n")
    }
}

pub(super) fn exported_compatibility_names(moved: &[&crate::parser::ParsedDeclaration]) -> String {
    let mut names = moved
        .iter()
        .filter(|declaration| declaration.is_exported)
        .map(|declaration| declaration.symbol_name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names.join(", ")
}

pub(super) fn compatibility_import_names(
    remaining_source: &str,
    moved_type_names: &BTreeSet<String>,
) -> Option<String> {
    let mut names = moved_type_names
        .iter()
        .filter(|name| {
            source_has_type_usage(remaining_source, &BTreeSet::from([(*name).clone()]))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    names.sort();
    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compatibility_type_statements(
    source: &str,
    source_path: &str,
    destination_path: &str,
    export_names: &str,
    import_names: Option<&str>,
    extension: &str,
    compatibility_quote_style: CompatibilityReexportQuoteStyle,
    has_external_referrers: bool,
    has_inline_type_referrers: bool,
) -> (String, String) {
    let module_specifier =
        relative_module_specifier_with_extension(source_path, destination_path, extension);
    let quote = module_specifier_quote(
        source,
        source_path,
        compatibility_quote_style,
        has_external_referrers,
        has_inline_type_referrers,
    );
    let semicolon = statement_semicolon(source);
    let quoted_specifier = format!("{quote}{module_specifier}{quote}");
    let import = import_names.map(|names| {
        format_named_from_statement(
            "import type",
            &split_specifier_names(names),
            &quoted_specifier,
            semicolon,
            source,
        )
    });
    let export = format_named_from_statement(
        "export type",
        &split_specifier_names(export_names),
        &quoted_specifier,
        semicolon,
        source,
    );
    (import.unwrap_or_default(), export)
}

fn split_specifier_names(names: &str) -> Vec<String> {
    names
        .split(", ")
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn module_specifier_quote(
    source: &str,
    source_path: &str,
    compatibility_quote_style: CompatibilityReexportQuoteStyle,
    has_external_referrers: bool,
    has_inline_type_referrers: bool,
) -> char {
    if has_inline_type_referrers {
        return '"';
    }
    if source_path.contains(".generated.") {
        return match compatibility_quote_style {
            CompatibilityReexportQuoteStyle::Extract => '"',
            CompatibilityReexportQuoteStyle::Apply => '\'',
        };
    }
    if compatibility_quote_style == CompatibilityReexportQuoteStyle::Extract
        && !has_external_referrers
    {
        return '"';
    }
    if source.contains("from \"") || source.contains("import \"") {
        '"'
    } else {
        '\''
    }
}

pub(super) struct SourceCompatibility<'a> {
    pub(super) source: &'a str,
    pub(super) source_path: &'a str,
    pub(super) destination_path: &'a str,
    pub(super) source_candidates: &'a [&'a Candidate],
    pub(super) moved: &'a [&'a ParsedDeclaration],
    pub(super) retained_private_names: &'a BTreeSet<String>,
    pub(super) cleanup_edits: &'a [TextEdit],
    pub(super) module_specifier_extension: &'a str,
    pub(super) compatibility_quote_style: CompatibilityReexportQuoteStyle,
    pub(super) has_external_referrers: bool,
    pub(super) has_inline_type_referrers: bool,
}

pub(super) fn plan_source_compatibility(context: SourceCompatibility<'_>) -> Result<Vec<TextEdit>> {
    let SourceCompatibility {
        source,
        source_path,
        destination_path,
        source_candidates,
        moved,
        retained_private_names,
        cleanup_edits,
        module_specifier_extension,
        compatibility_quote_style,
        has_external_referrers,
        has_inline_type_referrers,
    } = context;
    let source_edits;
    let source_is_empty = apply_text_edits(source, cleanup_edits)?.trim().is_empty();
    if source_is_empty {
        source_edits = vec![TextEdit {
            start: 0,
            end: source.len(),
            replacement: source_compatibility_exports(
                source,
                source_path,
                destination_path,
                moved,
                module_specifier_extension,
                false,
                None,
                compatibility_quote_style,
                has_external_referrers,
                has_inline_type_referrers,
            ),
        }];
    } else {
        let remaining_source = apply_text_edits(source, cleanup_edits)?;
        let moved_type_names = source_candidates
            .iter()
            .map(|candidate| candidate.symbol_name.clone())
            .chain(
                source_candidates
                    .iter()
                    .flat_map(|candidate| candidate.local_dependency_names.iter().cloned()),
            )
            .filter(|name| !retained_private_names.contains(name))
            .collect::<BTreeSet<_>>();
        let include_import = source_has_type_usage(&remaining_source, &moved_type_names)?;
        let export_names = exported_compatibility_names(moved);
        let import_names = compatibility_import_names(&remaining_source, &moved_type_names);
        if include_import {
            let (import_line, export_line) = compatibility_type_statements(
                &remaining_source,
                source_path,
                destination_path,
                &export_names,
                import_names.as_deref(),
                module_specifier_extension,
                compatibility_quote_style,
                has_external_referrers,
                has_inline_type_referrers,
            );
            source_edits = vec![TextEdit {
                start: 0,
                end: source.len(),
                replacement: place_remaining_source_compatibility(
                    &remaining_source,
                    &import_line,
                    &export_line,
                ),
            }];
        } else {
            let export_block = source_compatibility_exports(
                &remaining_source,
                source_path,
                destination_path,
                moved,
                module_specifier_extension,
                false,
                None,
                compatibility_quote_style,
                has_external_referrers,
                has_inline_type_referrers,
            );
            source_edits = vec![TextEdit {
                start: 0,
                end: source.len(),
                replacement: append_trailing_compatibility_exports(
                    &remaining_source,
                    &export_block,
                ),
            }];
        }
    }
    Ok(source_edits)
}
