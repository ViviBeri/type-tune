//! Edits for type re-exports in barrel modules.

use super::format::{quoted_module_specifier, statement_semicolon};
use crate::{edit::TextEdit, parser::ParsedReExport};
use std::collections::BTreeSet;

pub(super) fn rewrite_reexport_module_specifier(
    source: &str,
    export: &ParsedReExport,
    module_specifier: &str,
) -> TextEdit {
    let original_module_specifier =
        &source[export.module_specifier_start..export.module_specifier_end];
    let trailing = &source[export.module_specifier_end..export.statement_end];
    let has_import_attribute = trailing
        .split_once("with ")
        .is_some_and(|(leading, _)| leading.trim().is_empty());
    TextEdit {
        start: export.module_specifier_start,
        end: if has_import_attribute {
            export.statement_end
        } else {
            export.module_specifier_end
        },
        replacement: format!(
            "{}{}",
            quoted_module_specifier(original_module_specifier, module_specifier),
            if has_import_attribute && trailing.trim_end().ends_with(';') {
                ";"
            } else {
                ""
            },
        ),
    }
}

pub(super) fn inline_type_reexport_edits(
    re_exports: &[ParsedReExport],
    export: &ParsedReExport,
    source: &str,
) -> Vec<TextEdit> {
    let inline_prefix = "export { type ";
    let all_inline_type_only = re_exports
        .iter()
        .filter(|other| {
            other.statement_start == export.statement_start
                && other.statement_end == export.statement_end
        })
        .all(|other| other.is_inline_type_only);
    if !export.is_inline_type_only
        || !all_inline_type_only
        || !source[export.statement_start..export.statement_end].starts_with(inline_prefix)
    {
        return Vec::new();
    }
    let first_type_keyword_start = export.statement_start + "export { ".len();
    let mut subsequent_type_keywords = re_exports
        .iter()
        .filter(|other| {
            other.statement_start == export.statement_start
                && other.statement_end == export.statement_end
        })
        .filter_map(|other| {
            other
                .inline_type_keyword_start
                .zip(other.inline_type_keyword_end)
        })
        .filter(|(start, _)| *start != first_type_keyword_start)
        .collect::<Vec<_>>();
    subsequent_type_keywords.sort_unstable_by_key(|(start, _)| std::cmp::Reverse(*start));
    let mut edits = vec![TextEdit {
        start: export.statement_start,
        end: export.statement_start + inline_prefix.len(),
        replacement: "export type { ".into(),
    }];
    edits.extend(
        subsequent_type_keywords
            .into_iter()
            .map(|(start, end)| TextEdit {
                start,
                end: end + 1,
                replacement: String::new(),
            }),
    );
    edits
}

pub(super) fn split_mixed_inline_type_reexport(
    re_exports: &[ParsedReExport],
    export: &ParsedReExport,
    moved_symbols: &BTreeSet<String>,
    source: &str,
    destination_module_specifier: &str,
) -> Option<TextEdit> {
    if !export.is_inline_type_only || !moved_symbols.contains(&export.source_name) {
        return None;
    }
    let statement_exports = re_exports
        .iter()
        .filter(|other| {
            other.statement_start == export.statement_start
                && other.statement_end == export.statement_end
        })
        .collect::<Vec<_>>();
    let moved = statement_exports
        .iter()
        .filter(|other| other.is_inline_type_only && moved_symbols.contains(&other.source_name))
        .collect::<Vec<_>>();
    let remaining = statement_exports
        .iter()
        .filter(|other| !(other.is_inline_type_only && moved_symbols.contains(&other.source_name)))
        .collect::<Vec<_>>();
    if moved.is_empty() {
        return None;
    }
    let render_name = |re_export: &ParsedReExport| {
        if re_export.source_name == re_export.exported_name {
            re_export.source_name.clone()
        } else {
            format!("{} as {}", re_export.source_name, re_export.exported_name)
        }
    };
    let original_module_specifier =
        &source[export.module_specifier_start..export.module_specifier_end];
    let destination_module_specifier =
        quoted_module_specifier(original_module_specifier, destination_module_specifier);
    let moved_aliases = moved
        .iter()
        .filter(|re_export| re_export.source_name != re_export.exported_name)
        .collect::<Vec<_>>();
    let semicolon = statement_semicolon(source);
    let moved_re_exports = if !moved_aliases.is_empty() {
        format!(
            "export type {{ {} }} from {destination_module_specifier}{semicolon}",
            moved
                .iter()
                .filter(|re_export| re_export.source_name == re_export.exported_name)
                .chain(moved_aliases)
                .map(|re_export| render_name(re_export))
                .collect::<Vec<_>>()
                .join(", "),
        )
    } else {
        moved
            .iter()
            .map(|re_export| {
                format!(
                    "export type {{ {} }} from {destination_module_specifier}{semicolon}",
                    render_name(re_export)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let remaining_re_exports = (!remaining.is_empty()).then(|| {
        format!(
            "export {{ {} }} from {original_module_specifier}{semicolon}",
            remaining
                .iter()
                .map(|re_export| {
                    let name = render_name(re_export);
                    if re_export.is_inline_type_only {
                        format!("type {name}")
                    } else {
                        name
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
        )
    });
    Some(TextEdit {
        start: export.statement_start,
        end: export.statement_end,
        replacement: remaining_re_exports
            .map(|remaining| format!("{remaining}\n{moved_re_exports}"))
            .unwrap_or(moved_re_exports),
    })
}

/// Moves a standalone type re-export after later value re-exports, matching the TypeScript printer.
pub(super) fn move_type_only_reexport_after_values(
    re_exports: &[ParsedReExport],
    export: &ParsedReExport,
    source: &str,
    destination_module_specifier: &str,
) -> Option<Vec<TextEdit>> {
    let statement_exports = re_exports
        .iter()
        .filter(|other| {
            other.statement_start == export.statement_start
                && other.statement_end == export.statement_end
        })
        .collect::<Vec<_>>();
    if statement_exports.len() != 1
        || export.is_inline_type_only
        || !source[export.statement_start..export.statement_end]
            .trim_start()
            .starts_with("export type ")
    {
        return None;
    }
    let last_value_export = re_exports
        .iter()
        .filter(|other| {
            !source[other.statement_start..other.statement_end]
                .trim_start()
                .starts_with("export type ")
        })
        .max_by_key(|other| other.statement_start)?;
    if last_value_export.statement_start < export.statement_start {
        return None;
    }
    let module_specifier = quoted_module_specifier(
        &source[export.module_specifier_start..export.module_specifier_end],
        destination_module_specifier,
    );
    let moved_statement = format!(
        "{}{}{}",
        &source[export.statement_start..export.module_specifier_start],
        module_specifier,
        &source[export.module_specifier_end..export.statement_end],
    );
    let removal_end = if source[export.statement_end..].starts_with("\r\n") {
        export.statement_end + 2
    } else if source[export.statement_end..].starts_with('\n') {
        export.statement_end + 1
    } else {
        export.statement_end
    };
    Some(vec![
        TextEdit {
            start: export.statement_start,
            end: removal_end,
            replacement: String::new(),
        },
        TextEdit {
            start: last_value_export.statement_end,
            end: last_value_export.statement_end,
            replacement: format!("\n{moved_statement}"),
        },
    ])
}
