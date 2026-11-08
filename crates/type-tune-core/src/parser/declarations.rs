//! Collect declarations, dependencies, and unsupported type names.

use std::collections::BTreeSet;

use tree_sitter::Node;

use super::ParsedDeclaration;

pub(super) fn collect_declarations(
    node: Node<'_>,
    source: &[u8],
    exported: bool,
    default_export: bool,
    exported_start: Option<usize>,
    declarations: &mut Vec<ParsedDeclaration>,
) {
    let exported = exported || node.kind() == "export_statement";
    let default_export = default_export
        || (node.kind() == "export_statement"
            && node
                .utf8_text(source)
                .is_ok_and(|text| text.trim_start().starts_with("export default")));
    let exported_start = if node.kind() == "export_statement" {
        Some(node.start_byte())
    } else {
        exported_start
    };
    if matches!(
        node.kind(),
        "type_alias_declaration" | "interface_declaration"
    ) && let Some(name) = node.child_by_field_name("name")
    {
        let declaration_start = exported_start.unwrap_or(node.start_byte());
        let leading_comment_start_byte = leading_comment_start(source, declaration_start);
        let (referenced_type_names, has_type_query) = collect_type_reference_info(node, source);
        declarations.push(ParsedDeclaration {
            symbol_name: name.utf8_text(source).unwrap_or_default().into(),
            kind: if node.kind() == "type_alias_declaration" {
                "typeAlias".into()
            } else {
                "interface".into()
            },
            start_position: utf16_offset(source, declaration_start),
            start_byte: declaration_start,
            leading_comment_start_byte,
            end_byte: node.end_byte(),
            declaration_text: String::from_utf8_lossy(&source[declaration_start..node.end_byte()])
                .into_owned(),
            leading_comment_text: String::from_utf8_lossy(
                &source[leading_comment_start_byte..declaration_start],
            )
            .trim()
            .into(),
            is_exported: exported,
            is_default_export: default_export,
            referenced_type_names,
            has_type_query,
        });
    }
    if node.kind() == "internal_module" {
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_declarations(
            child,
            source,
            exported,
            default_export,
            exported_start,
            declarations,
        );
    }
}

fn leading_comment_start(source: &[u8], declaration_start: usize) -> usize {
    let text = String::from_utf8_lossy(&source[..declaration_start]);
    let mut cursor = text.len();
    let mut comment_start = declaration_start;
    loop {
        let trimmed = text[..cursor].trim_end_matches(char::is_whitespace).len();
        // A blank line separates a file or section banner from the declaration's own comment.
        if text[trimmed..cursor].matches('\n').count() >= 2 {
            return comment_start;
        }
        cursor = trimmed;
        let before = &text[..cursor];
        if let Some(block_end) = before.strip_suffix("*/")
            && let Some(start) = block_end.rfind("/*")
        {
            comment_start = start;
            cursor = start;
            continue;
        }
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        let line = &before[line_start..];
        if let Some(offset) = line.find("//")
            && line[..offset].trim().is_empty()
        {
            comment_start = line_start + offset;
            cursor = comment_start;
            continue;
        }
        return comment_start;
    }
}

pub(super) fn collect_unsupported_type_names(
    node: Node<'_>,
    source: &[u8],
    names: &mut BTreeSet<String>,
) {
    if matches!(
        node.kind(),
        "abstract_class_declaration" | "class_declaration" | "enum_declaration" | "internal_module"
    ) && let Some(name) = node.child_by_field_name("name")
        && let Ok(name) = name.utf8_text(source)
    {
        names.insert(name.into());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_unsupported_type_names(child, source, names);
    }
}

fn is_typeof_import_type_query(node: Node<'_>, source: &[u8]) -> bool {
    if node.kind() != "type_query" {
        return false;
    }
    let mut found_import_type = false;
    visit_nodes(node, &mut |child| {
        if child.kind() == "import_type" {
            found_import_type = true;
        }
    });
    found_import_type
        || node
            .utf8_text(source)
            .is_ok_and(|text| text.contains("import("))
}

fn visit_nodes(node: Node<'_>, visit: &mut dyn FnMut(Node<'_>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_nodes(child, visit);
    }
}

fn collect_type_reference_info(node: Node<'_>, source: &[u8]) -> (Vec<String>, bool) {
    let mut names = Vec::new();
    let mut has_type_query = false;
    collect_type_identifiers_from_node(node, source, &mut names, &mut has_type_query);
    names.sort();
    names.dedup();
    (names, has_type_query)
}

fn collect_type_identifiers_from_node(
    node: Node<'_>,
    source: &[u8],
    names: &mut Vec<String>,
    has_type_query: &mut bool,
) {
    if node.kind() == "type_query" && !is_typeof_import_type_query(node, source) {
        *has_type_query = true;
    }
    if node.kind() == "nested_type_identifier"
        && let Some(module) = node.child_by_field_name("module")
        && let Ok(module_name) = module.utf8_text(source)
        && let Some(namespace_name) = module_name.split('.').next()
    {
        names.push(namespace_name.into());
    }
    if node.kind() == "type_identifier"
        && let Ok(name) = node.utf8_text(source)
    {
        names.push(name.into());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_type_identifiers_from_node(child, source, names, has_type_query);
    }
}

fn utf16_offset(source: &[u8], byte_offset: usize) -> usize {
    std::str::from_utf8(&source[..byte_offset])
        .map(|prefix| prefix.encode_utf16().count())
        .unwrap_or(byte_offset)
}
