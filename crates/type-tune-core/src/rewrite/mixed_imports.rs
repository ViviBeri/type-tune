//! Plan splits of referrer imports containing both moved and retained bindings.

use std::collections::HashSet;

use crate::{
    edit::TextEdit,
    model::{Candidate, ReferrerStrategy},
    parser::ParsedImportAnalysis,
};

use super::{
    format::{
        after_trailing_import_comment_line, earlier_sorted_import_offset,
        earlier_split_import_offset, format_named_import_clause, later_import_insert_offset,
        named_import_has_comments, quoted_module_specifier, statement_semicolon,
        trailing_import_comment_end,
    },
    referrer_edits::{ReferrerEditContext, destination_specifier_for_import},
    resolution::resolve_referrer_import,
};

pub(super) fn plan_mixed_import_edits(
    context: ReferrerEditContext<'_>,
    candidate: &Candidate,
    referrer: &ReferrerStrategy,
    source: &str,
    import_analysis: &ParsedImportAnalysis,
) -> Vec<TextEdit> {
    let candidates_by_source = context.candidates_by_source;
    let mut mixed_import_edits = Vec::new();
    let mut pending_inserts = Vec::new();
    let mut mixed_import_ranges = HashSet::new();
    let moved_names = candidates_by_source
        .get(&candidate.source_path)
        .map(|candidates| {
            candidates
                .iter()
                .map(|candidate| candidate.symbol_name.as_str())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    for import in &import_analysis.imports {
        if import.is_default || import.is_namespace || import.imported_name != candidate.symbol_name
        {
            continue;
        }
        let resolved_path = resolve_referrer_import(context, referrer, &import.module_specifier);
        if resolved_path.as_deref() != Some(candidate.source_path.as_str())
            || !mixed_import_ranges.insert((import.statement_start, import.statement_end))
        {
            continue;
        }
        let bindings = import_analysis
            .imports
            .iter()
            .filter(|binding| {
                binding.statement_start == import.statement_start
                    && binding.statement_end == import.statement_end
            })
            .collect::<Vec<_>>();
        if bindings.len() < 2 || bindings.iter().any(|binding| binding.is_namespace) {
            continue;
        }
        let (moved, remaining): (Vec<_>, Vec<_>) = bindings.into_iter().partition(|binding| {
            binding.is_type_only
                && !binding.is_default
                && !binding.is_namespace
                && moved_names.contains(binding.imported_name.as_str())
        });
        if moved.is_empty() || remaining.is_empty() {
            continue;
        }
        let original_statement = &source[import.statement_start..import.statement_end];
        if named_import_has_comments(original_statement) {
            // The source compatibility re-export keeps this commented import valid.
            mixed_import_edits.push(TextEdit {
                start: import.statement_start,
                end: import.statement_end,
                replacement: original_statement.to_owned(),
            });
            continue;
        }
        let module_specifier = destination_specifier_for_import(
            context,
            candidate,
            &referrer.referrer_path,
            &import.module_specifier,
        );
        let preserves_declaration_type_modifier = source
            [import.statement_start..import.statement_end]
            .trim_start()
            .starts_with("import type")
            && remaining.iter().all(|binding| binding.is_type_only);
        let render_binding = |binding: &&crate::parser::ParsedImport| {
            let prefix = if binding.is_type_only && !preserves_declaration_type_modifier {
                "type "
            } else {
                ""
            };
            if binding.imported_name == binding.local_name {
                format!("{prefix}{}", binding.imported_name)
            } else {
                format!(
                    "{prefix}{} as {}",
                    binding.imported_name, binding.local_name
                )
            }
        };
        let original_module_specifier = quoted_module_specifier(
            &source[import.module_specifier_start..import.module_specifier_end],
            &import.module_specifier,
        );
        let default_import = remaining.iter().find(|binding| binding.is_default);
        let named_imports = remaining
            .iter()
            .filter(|binding| !binding.is_default)
            .map(render_binding)
            .collect::<Vec<_>>();
        let remaining_clause = match (default_import, named_imports.is_empty()) {
            (Some(default_import), true) => {
                let prefix = if default_import.is_type_only && !preserves_declaration_type_modifier
                {
                    "type "
                } else {
                    ""
                };
                format!("{prefix}{}", default_import.local_name)
            }
            (Some(default_import), false) => {
                let prefix = if default_import.is_type_only && !preserves_declaration_type_modifier
                {
                    "type "
                } else {
                    ""
                };
                format!(
                    "{prefix}{}, {{ {} }}",
                    default_import.local_name,
                    named_imports.join(", ")
                )
            }
            (None, false) => {
                let semicolon = statement_semicolon(source);
                let trailer = &source[import.module_specifier_end..import.statement_end];
                let trailer = if trailer.trim_start().starts_with("with ") {
                    trailer
                } else {
                    semicolon
                };
                let keyword = if preserves_declaration_type_modifier {
                    "import type"
                } else {
                    "import"
                };
                format_named_import_clause(
                    &named_imports,
                    &source[import.statement_start..import.statement_end],
                    format!("{keyword}  from {original_module_specifier}{trailer}")
                        .chars()
                        .count(),
                )
            }
            (None, true) => continue,
        };
        let semicolon = statement_semicolon(source);
        let statement_key = (
            referrer.referrer_path.clone(),
            import.statement_start,
            import.statement_end,
        );
        let absorbed = context.absorbed_partials.contains(&statement_key);
        let mut moved_names_text = moved
            .iter()
            .map(|binding| {
                if binding.imported_name == binding.local_name {
                    binding.imported_name.clone()
                } else {
                    format!("{} as {}", binding.imported_name, binding.local_name)
                }
            })
            .collect::<Vec<_>>();
        if let Some(extra) = context.extra_partial_names.get(&statement_key) {
            moved_names_text.extend(extra.iter().cloned());
        }
        let new_type_import = (!absorbed).then(|| {
            let quoted = quoted_module_specifier(
                &source[import.module_specifier_start..import.module_specifier_end],
                &module_specifier,
            );
            let semicolon = statement_semicolon(source);
            let clause = format_named_import_clause(
                &moved_names_text,
                &source[import.statement_start..import.statement_end],
                format!("import type  from {quoted}{semicolon}")
                    .chars()
                    .count(),
            );
            format!("import type {clause} from {quoted}{semicolon}")
        });
        let following = &source[import.statement_end..];
        // The newline after the statement stays outside the edit. Add a blank line before the next
        // statement, and leave an existing gap or the next import in the same block untouched.
        let next_code = following.trim_start_matches(['\n', '\r']);
        let trailing_separator = if following.starts_with("\n\n")
            || following.starts_with("\r\n\r\n")
            || following.starts_with("\r\n\n")
            || next_code.starts_with("import ")
        {
            ""
        } else if following.starts_with(['\n', '\r']) {
            "\n"
        } else {
            "\n\n"
        };
        let original_import_trailer = &source[import.module_specifier_end..import.statement_end];
        let original_import_trailer = if original_import_trailer.trim_start().starts_with("with ") {
            original_import_trailer
        } else {
            semicolon
        };
        let keyword = if preserves_declaration_type_modifier {
            "import type"
        } else {
            "import"
        };
        let insert_at = new_type_import.as_deref().and_then(|statement| {
            earlier_sorted_import_offset(
                source,
                import.statement_start,
                import.statement_end,
                statement,
            )
            .or_else(|| {
                earlier_split_import_offset(
                    source,
                    import.statement_start,
                    &source[import.statement_start..import.statement_end],
                    statement,
                )
            })
            .or_else(|| after_trailing_import_comment_line(source, import.statement_end))
            .or_else(|| {
                trailing_import_comment_end(source, import.statement_end)
                    .map(|_| import.statement_start)
            })
            .or_else(|| later_import_insert_offset(source, import.statement_end, statement))
        });
        let replacement = match (&new_type_import, insert_at) {
            (Some(new_type_import), None) => format!(
                "{keyword} {remaining_clause} from {original_module_specifier}{original_import_trailer}\n{new_type_import}{trailing_separator}"
            ),
            _ => format!(
                "{keyword} {remaining_clause} from {original_module_specifier}{original_import_trailer}"
            ),
        };
        if let (Some(statement), Some(at)) = (new_type_import, insert_at) {
            pending_inserts.push((at, statement));
        }
        mixed_import_edits.push(TextEdit {
            start: import.statement_start,
            end: import.statement_end,
            replacement,
        });
    }
    for (at, statement) in pending_inserts {
        if let Some(edit) = mixed_import_edits.iter_mut().find(|edit| edit.start == at) {
            edit.replacement = format!("{statement}\n{}", edit.replacement);
        } else if let Some(edit) = mixed_import_edits
            .iter_mut()
            .find(|edit| at > edit.start && at < edit.end)
        {
            edit.replacement.push('\n');
            edit.replacement.push_str(&statement);
        } else {
            mixed_import_edits.push(TextEdit {
                start: at,
                end: at,
                replacement: format!("{statement}\n"),
            });
        }
    }
    mixed_import_edits
}
