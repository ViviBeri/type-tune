//! Collect static and inline type imports from TypeScript syntax trees.

use tree_sitter::Node;

use super::{ParsedImport, ParsedInlineTypeImport, first_string_child};

struct ImportStatementContext<'a> {
    module_specifier: &'a str,
    is_type_only: bool,
    module_specifier_start: usize,
    module_specifier_end: usize,
    statement_start: usize,
    statement_end: usize,
}

pub(super) fn collect_imports(node: Node<'_>, source: &[u8], imports: &mut Vec<ParsedImport>) {
    if node.kind() == "import_statement" {
        let module_specifier = node
            .child_by_field_name("source")
            .and_then(|value| value.utf8_text(source).ok())
            .map(|value| value.trim_matches(['\'', '"']).to_owned());
        if let Some(module_specifier) = module_specifier {
            let source_node = node.child_by_field_name("source");
            let (module_specifier_start, module_specifier_end) = source_node
                .map(|source| (source.start_byte(), source.end_byte()))
                .unwrap_or_default();
            let is_type_only = node
                .utf8_text(source)
                .is_ok_and(|text| text.trim_start().starts_with("import type"));
            let context = ImportStatementContext {
                module_specifier: &module_specifier,
                is_type_only,
                module_specifier_start,
                module_specifier_end,
                statement_start: node.start_byte(),
                statement_end: node.end_byte(),
            };
            let mut cursor = node.walk();
            for descendant in node.children(&mut cursor) {
                collect_import_specifiers(descendant, source, &context, imports);
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_imports(child, source, imports);
    }
}

pub(super) fn collect_inline_type_imports(
    node: Node<'_>,
    source: &[u8],
    imports: &mut Vec<ParsedInlineTypeImport>,
) {
    if node.kind() == "member_expression"
        && is_type_position(node)
        && let Some(call) = node.child_by_field_name("object")
        && call.kind() == "call_expression"
        && call
            .child_by_field_name("function")
            .is_some_and(|function| function.kind() == "import")
        && let (Some(module_specifier_node), Some(imported_name_node)) = (
            first_string_child(call),
            node.child_by_field_name("property"),
        )
        && let (Ok(module_specifier), Ok(imported_name)) = (
            module_specifier_node.utf8_text(source),
            imported_name_node.utf8_text(source),
        )
    {
        imports.push(ParsedInlineTypeImport {
            imported_name: imported_name.into(),
            module_specifier: module_specifier.trim_matches(['\'', '"']).into(),
            module_specifier_start: module_specifier_node.start_byte(),
            module_specifier_end: module_specifier_node.end_byte(),
        });
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_inline_type_imports(child, source, imports);
    }
}

fn is_type_position(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "type_alias_declaration"
            | "interface_declaration"
            | "type_annotation"
            | "type_arguments"
            | "type_query" => return true,
            _ => node = parent,
        }
    }
    false
}

fn collect_import_specifiers(
    node: Node<'_>,
    source: &[u8],
    context: &ImportStatementContext<'_>,
    imports: &mut Vec<ParsedImport>,
) {
    if node.kind() == "import_clause" {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "identifier" {
                let local_name = child.utf8_text(source).unwrap_or_default();
                imports.push(ParsedImport {
                    imported_name: "default".into(),
                    local_name: local_name.into(),
                    is_default: true,
                    is_namespace: false,
                    module_specifier: context.module_specifier.into(),
                    is_type_only: context.is_type_only,
                    module_specifier_start: context.module_specifier_start,
                    module_specifier_end: context.module_specifier_end,
                    statement_start: context.statement_start,
                    statement_end: context.statement_end,
                });
            }
        }
    }
    if node.kind() == "import_specifier" {
        let imported_name = node
            .child_by_field_name("name")
            .or_else(|| node.named_child(0));
        let local_name = node.child_by_field_name("alias").or(imported_name);
        if let (Some(imported_name), Some(local_name)) = (imported_name, local_name) {
            imports.push(ParsedImport {
                imported_name: imported_name.utf8_text(source).unwrap_or_default().into(),
                local_name: local_name.utf8_text(source).unwrap_or_default().into(),
                is_default: false,
                is_namespace: false,
                module_specifier: context.module_specifier.into(),
                is_type_only: context.is_type_only
                    || node
                        .utf8_text(source)
                        .is_ok_and(|text| text.trim_start().starts_with("type ")),
                module_specifier_start: context.module_specifier_start,
                module_specifier_end: context.module_specifier_end,
                statement_start: context.statement_start,
                statement_end: context.statement_end,
            });
        }
    }
    if node.kind() == "namespace_import"
        && let Some(local_name) = node.named_child(0)
    {
        imports.push(ParsedImport {
            imported_name: "*".into(),
            local_name: local_name.utf8_text(source).unwrap_or_default().into(),
            is_default: false,
            is_namespace: true,
            module_specifier: context.module_specifier.into(),
            is_type_only: context.is_type_only,
            module_specifier_start: context.module_specifier_start,
            module_specifier_end: context.module_specifier_end,
            statement_start: context.statement_start,
            statement_end: context.statement_end,
        });
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_import_specifiers(child, source, context, imports);
    }
}
