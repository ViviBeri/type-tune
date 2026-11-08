//! Collect TypeScript re-exports and their source offsets.

use std::sync::OnceLock;

use regex::Regex;
use tree_sitter::Node;

use super::{ParsedReExport, first_string_child};

struct ReExportStatementContext<'a> {
    module_specifier: &'a str,
    module_specifier_start: usize,
    module_specifier_end: usize,
    statement_start: usize,
    statement_end: usize,
}

pub(super) fn collect_re_exports(node: Node<'_>, source: &[u8], exports: &mut Vec<ParsedReExport>) {
    if node.kind() == "export_statement" {
        let statement_text = node.utf8_text(source).unwrap_or_default();
        if node.child_by_field_name("source").is_none()
            && let Some(source_node) = first_string_child(node)
            && collect_attributed_re_exports(node, source, source_node, exports)
        {
            return;
        }
        let type_namespace_name = statement_text
            .trim_start()
            .strip_prefix("export type * as ")
            .and_then(|text| text.split_whitespace().next());
        let namespace_name = type_namespace_name.or_else(|| {
            statement_text
                .trim_start()
                .strip_prefix("export * as ")
                .and_then(|text| text.split_whitespace().next())
        });
        let is_type_wildcard = statement_text
            .trim_start()
            .starts_with("export type * from ")
            || type_namespace_name.is_some();
        let module_specifier = node
            .child_by_field_name("source")
            .and_then(|value| value.utf8_text(source).ok())
            .map(|value| value.trim_matches(['\'', '"']).to_owned());
        if let Some(module_specifier) = module_specifier {
            let source_node = node.child_by_field_name("source");
            let (module_specifier_start, module_specifier_end) = source_node
                .map(|source| (source.start_byte(), source.end_byte()))
                .unwrap_or_default();
            if node.named_child_count() == 1 || is_type_wildcard || namespace_name.is_some() {
                exports.push(ParsedReExport {
                    exported_name: namespace_name.unwrap_or("*").into(),
                    source_name: "*".into(),
                    is_wildcard: true,
                    is_type_wildcard,
                    is_type_namespace_wildcard: type_namespace_name.is_some(),
                    is_namespace_wildcard: namespace_name.is_some(),
                    is_inline_type_only: false,
                    inline_type_keyword_start: None,
                    inline_type_keyword_end: None,
                    module_specifier,
                    module_specifier_start,
                    module_specifier_end,
                    statement_start: node.start_byte(),
                    statement_end: node.end_byte(),
                });
            } else {
                let context = ReExportStatementContext {
                    module_specifier: &module_specifier,
                    module_specifier_start,
                    module_specifier_end,
                    statement_start: node.start_byte(),
                    statement_end: node.end_byte(),
                };
                let mut cursor = node.walk();
                for descendant in node.children(&mut cursor) {
                    collect_export_specifiers(descendant, source, &context, exports);
                }
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_re_exports(child, source, exports);
    }
}

pub(super) fn collect_attributed_type_wildcard_re_exports(
    source: &str,
    exports: &mut Vec<ParsedReExport>,
) {
    // tree-sitter-typescript 0.23 does not emit an `export_statement` node for this
    // otherwise valid TypeScript syntax, so the AST collector above cannot see it.
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(
            r#"export\s+type\s+\*\s+(?:as\s+(?P<namespace>[A-Za-z_$][A-Za-z0-9_$]*)\s+)?from\s+(?P<module>['\"][^'\"]+['\"])\s+with\s*\{"#,
        )
            .expect("attributed wildcard re-export pattern is valid")
    });
    for captures in pattern.captures_iter(source) {
        let Some(module) = captures.name("module") else {
            continue;
        };
        let namespace_name = captures.name("namespace").map(|capture| capture.as_str());
        if exports
            .iter()
            .any(|export| export.module_specifier_start == module.start())
        {
            continue;
        }
        let statement_start = captures
            .get(0)
            .map_or(module.start(), |capture| capture.start());
        let trailing = &source[module.end()..];
        let statement_end = trailing
            .find([';', '\n', '\r'])
            .map_or(source.len(), |offset| module.end() + offset + 1);
        exports.push(ParsedReExport {
            exported_name: namespace_name.unwrap_or("*").into(),
            source_name: "*".into(),
            is_wildcard: true,
            is_type_wildcard: true,
            is_type_namespace_wildcard: namespace_name.is_some(),
            is_namespace_wildcard: namespace_name.is_some(),
            is_inline_type_only: false,
            inline_type_keyword_start: None,
            inline_type_keyword_end: None,
            module_specifier: module.as_str().trim_matches(['\'', '"']).into(),
            module_specifier_start: module.start(),
            module_specifier_end: module.end(),
            statement_start,
            statement_end,
        });
    }
}

fn collect_attributed_re_exports(
    node: Node<'_>,
    source: &[u8],
    source_node: Node<'_>,
    exports: &mut Vec<ParsedReExport>,
) -> bool {
    let statement_text = node.utf8_text(source).unwrap_or_default();
    let Some(export_text) = statement_text.split_once(" with ").map(|(text, _)| text) else {
        return false;
    };
    let export_text = export_text.trim_start();
    let Ok(module_specifier) = source_node.utf8_text(source) else {
        return false;
    };
    let module_specifier = module_specifier.trim_matches(['\'', '"']);
    if export_text.starts_with("export type * from ") {
        exports.push(ParsedReExport {
            exported_name: "*".into(),
            source_name: "*".into(),
            is_wildcard: true,
            is_type_wildcard: true,
            is_type_namespace_wildcard: false,
            is_namespace_wildcard: false,
            is_inline_type_only: false,
            inline_type_keyword_start: None,
            inline_type_keyword_end: None,
            module_specifier: module_specifier.into(),
            module_specifier_start: source_node.start_byte(),
            module_specifier_end: source_node.end_byte(),
            statement_start: node.start_byte(),
            statement_end: node.end_byte(),
        });
        return true;
    }
    let Some(bindings) = export_text
        .strip_prefix("export type {")
        .and_then(|text| text.split_once("} from ").map(|(bindings, _)| bindings))
    else {
        return false;
    };
    for binding in bindings.split(',') {
        let binding = binding
            .trim()
            .strip_prefix("type ")
            .unwrap_or(binding.trim());
        let (source_name, exported_name) = binding
            .split_once(" as ")
            .map_or((binding, binding), |(source_name, exported_name)| {
                (source_name.trim(), exported_name.trim())
            });
        if source_name.is_empty() || exported_name.is_empty() {
            return false;
        }
        exports.push(ParsedReExport {
            exported_name: exported_name.into(),
            source_name: source_name.into(),
            is_wildcard: false,
            is_type_wildcard: false,
            is_type_namespace_wildcard: false,
            is_namespace_wildcard: false,
            is_inline_type_only: false,
            inline_type_keyword_start: None,
            inline_type_keyword_end: None,
            module_specifier: module_specifier.into(),
            module_specifier_start: source_node.start_byte(),
            module_specifier_end: source_node.end_byte(),
            statement_start: node.start_byte(),
            statement_end: node.end_byte(),
        });
    }
    true
}

fn collect_export_specifiers(
    node: Node<'_>,
    source: &[u8],
    context: &ReExportStatementContext<'_>,
    exports: &mut Vec<ParsedReExport>,
) {
    if node.kind() == "export_specifier" {
        let source_name = node
            .child_by_field_name("name")
            .or_else(|| node.named_child(0));
        let exported_name = node.child_by_field_name("alias").or(source_name);
        if let (Some(source_name), Some(exported_name)) = (source_name, exported_name) {
            let inline_type_keyword_start = node.utf8_text(source).ok().and_then(|text| {
                text.trim_start()
                    .starts_with("type ")
                    .then_some(node.start_byte() + text.len() - text.trim_start().len())
            });
            exports.push(ParsedReExport {
                exported_name: exported_name.utf8_text(source).unwrap_or_default().into(),
                source_name: source_name.utf8_text(source).unwrap_or_default().into(),
                is_wildcard: false,
                is_type_wildcard: false,
                is_type_namespace_wildcard: false,
                is_namespace_wildcard: false,
                is_inline_type_only: inline_type_keyword_start.is_some(),
                inline_type_keyword_start,
                inline_type_keyword_end: inline_type_keyword_start
                    .map(|start| start + "type".len()),
                module_specifier: context.module_specifier.into(),
                module_specifier_start: context.module_specifier_start,
                module_specifier_end: context.module_specifier_end,
                statement_start: context.statement_start,
                statement_end: context.statement_end,
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_export_specifiers(child, source, context, exports);
    }
}
