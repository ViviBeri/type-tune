//! Plan removal of moved declarations and cleanup of their imports.

use std::collections::HashSet;

use crate::{
    edit::TextEdit,
    parser::{ParsedDeclaration, ParsedSource},
};

use super::imports::source_import_cleanup_replacement;

pub(super) fn plan_source_cleanup(
    source: &str,
    parsed_source: &ParsedSource,
    moved: &[&ParsedDeclaration],
    statement_ranges: &HashSet<(usize, usize)>,
    kept_import_names: &HashSet<String>,
) -> Vec<TextEdit> {
    let local_type_names = parsed_source
        .declarations
        .iter()
        .map(|declaration| declaration.symbol_name.as_str())
        .collect::<HashSet<_>>();
    let moved_type_names = moved
        .iter()
        .flat_map(|declaration| declaration.referenced_type_names.iter())
        .filter(|name| {
            !local_type_names.contains(name.as_str()) && !kept_import_names.contains(name.as_str())
        })
        .cloned()
        .collect::<HashSet<_>>();
    let mut edits = moved
        .iter()
        .map(|declaration| declaration_removal_edit(source, declaration))
        .collect::<Vec<_>>();
    edits.extend(statement_ranges.iter().map(|(start, end)| TextEdit {
        start: *start,
        end: *end,
        replacement: source_import_cleanup_replacement(
            source,
            parsed_source,
            *start,
            *end,
            &moved_type_names,
        ),
    }));
    edits
}

pub(super) fn declaration_removal_edit(source: &str, declaration: &ParsedDeclaration) -> TextEdit {
    TextEdit {
        start: declaration.leading_comment_start_byte,
        end: removal_end(source, declaration.end_byte),
        replacement: String::new(),
    }
}

fn removal_end(source: &str, declaration_end: usize) -> usize {
    declaration_end + source[declaration_end..].len() - source[declaration_end..].trim_start().len()
}
