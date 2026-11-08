//! Formatting helpers for rewritten source imports.

use crate::{
    module_specifier::{is_relative_module_specifier, preserved_module_specifier_extension},
    parser::ParsedSource,
};

/// Use CRLF for generated text only when the original file consistently uses CRLF.
pub(super) fn match_line_endings(source: &str, generated: &str) -> String {
    if source.contains("\r\n") && !source.replace("\r\n", "").contains('\n') {
        generated.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        generated.to_owned()
    }
}

pub(super) fn module_specifier_extension_for_source(
    source: &ParsedSource,
    default_extension: &str,
) -> String {
    source
        .module_specifiers()
        .filter(|specifier| is_relative_module_specifier(specifier))
        .find_map(preserved_module_specifier_extension)
        .unwrap_or(default_extension)
        .to_owned()
}

pub(super) fn quoted_module_specifier(original: &str, replacement: &str) -> String {
    let quote = original
        .chars()
        .next()
        .filter(|character| matches!(character, '\'' | '"'));
    quote.map_or_else(
        || format!("\"{replacement}\""),
        |quote| format!("{quote}{replacement}{quote}"),
    )
}

pub(super) fn split_leading_import_block_with_gap(source: &str) -> (String, String, bool) {
    collect_leading_import_parts(source)
}

/// Comments and directives that stay above the import block.
///
/// A comment is part of the header when a blank line separates it from the next statement, when it
/// sits directly above an import, or when it is a file-level directive such as `eslint-disable`.
/// A comment on the following declaration stays with that declaration.
pub(super) fn split_leading_file_header(source: &str) -> (&str, &str) {
    let mut index = 0;
    if let Some(rest) = source.strip_prefix("#!") {
        index = rest
            .find('\n')
            .map_or(source.len(), |newline| 2 + newline + 1);
    }
    loop {
        let after_space = index + source[index..].len() - source[index..].trim_start().len();
        let Some(comment_end) = comment_end_at(source, after_space) else {
            return (&source[..index], &source[index..]);
        };
        let after = &source[comment_end..];
        let whitespace_len = after.len() - after.trim_start().len();
        let newlines = after[..whitespace_len]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        let next = &source[comment_end + whitespace_len..];
        let above_import = next.starts_with("import ") || next.starts_with("import{");
        if newlines >= 2
            || above_import
            || is_file_header_directive(&source[after_space..comment_end])
        {
            index = comment_end + whitespace_len;
            continue;
        }
        return (&source[..index], &source[index..]);
    }
}

/// Collects the leading import block. `separated` is true when a blank line follows it.
fn collect_leading_import_parts(source: &str) -> (String, String, bool) {
    let mut rest = source.trim_start();
    let mut statements = Vec::new();
    let mut separated = false;
    while rest.starts_with("import ") || rest.starts_with("import{") {
        let Some(end) = import_statement_end(rest) else {
            break;
        };
        statements.push(rest[..=end].trim().to_string());
        let after = &rest[end + 1..];
        let whitespace_len = after.len() - after.trim_start().len();
        separated = after[..whitespace_len]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            >= 2;
        rest = after.trim_start();
    }
    if statements.is_empty() {
        return (String::new(), source.trim().to_owned(), false);
    }
    let imports = format!("{}\n", statements.join("\n"));
    (imports, rest.trim().to_owned(), separated)
}

/// Insert one import without reconstructing existing statements or their whitespace.
pub(super) fn insert_leading_import(source: &str, statement: &str) -> String {
    let (header, _) = split_leading_file_header(source);
    let mut offset = header.len();
    let mut imports = Vec::new();
    loop {
        let rest = &source[offset..];
        let start = offset + rest.len() - rest.trim_start().len();
        let rest = &source[start..];
        if !(rest.starts_with("import ") || rest.starts_with("import{")) {
            break;
        }
        let Some(end) = import_statement_end(rest) else {
            break;
        };
        let mut end = start + end + 1;
        while end > start && matches!(source.as_bytes()[end - 1], b'\r' | b'\n') {
            end -= 1;
        }
        end = trailing_import_comment_end(source, end).unwrap_or(end);
        imports.push((start, end));
        offset = end;
    }
    if imports.is_empty() {
        let separator = if source[header.len()..].trim().is_empty() {
            "\n"
        } else {
            "\n\n"
        };
        return format!(
            "{}{statement}{separator}{}",
            &source[..header.len()],
            &source[header.len()..]
        );
    }
    let new_key = specifier_sort_key(statement);
    let same_group = imports
        .iter()
        .filter(|(start, end)| specifier_sort_key(&source[*start..*end]).0 == new_key.0)
        .collect::<Vec<_>>();
    if let Some((start, _)) = same_group
        .iter()
        .find(|(start, end)| specifier_sort_key(&source[*start..*end]) > new_key)
        && !(*start == imports[0].0
            && !header.trim().is_empty()
            && header[header.trim_end().len()..]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                < 2)
    {
        return format!("{}{statement}\n{}", &source[..*start], &source[*start..]);
    }
    let (_, end) = same_group
        .last()
        .copied()
        .unwrap_or_else(|| imports.last().expect("imports exist"));
    let has_groups = imports.windows(2).any(|pair| {
        source[pair[0].1..pair[1].0]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            >= 2
    });
    let separator = if same_group.is_empty() && has_groups {
        "\n\n"
    } else {
        "\n"
    };
    format!(
        "{}{separator}{statement}{}",
        &source[..*end],
        &source[*end..]
    )
}

pub(super) fn ensure_blank_after_leading_imports(source: &str) -> String {
    let (header, _) = split_leading_file_header(source);
    let mut offset = header.len();
    let mut last_end = None;
    loop {
        let rest = &source[offset..];
        let start = offset + rest.len() - rest.trim_start().len();
        let rest = &source[start..];
        if !(rest.starts_with("import ") || rest.starts_with("import{")) {
            break;
        }
        let Some(end) = import_statement_end(rest) else {
            break;
        };
        let mut end = start + end + 1;
        while end > start && matches!(source.as_bytes()[end - 1], b'\r' | b'\n') {
            end -= 1;
        }
        end = trailing_import_comment_end(source, end).unwrap_or(end);
        last_end = Some(end);
        offset = end;
    }
    let Some(end) = last_end else {
        return source.to_owned();
    };
    let after = &source[end..];
    let whitespace_len = after.len() - after.trim_start().len();
    if !after[whitespace_len..].is_empty()
        && after[..whitespace_len]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            < 2
    {
        return format!("{}\n{}", &source[..end], &source[end..]);
    }
    source.to_owned()
}

/// Prefer placing a split import before its source when it sorts first in the same group.
pub(super) fn earlier_split_import_offset(
    source: &str,
    start: usize,
    original: &str,
    new_statement: &str,
) -> Option<usize> {
    let old_key = specifier_sort_key(original);
    let new_key = specifier_sort_key(new_statement);
    if old_key.0 != new_key.0 || new_key >= old_key {
        return None;
    }
    let before = &source[..start];
    let previous_line = before
        .trim_end_matches(['\r', '\n'])
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .trim();
    if previous_line.starts_with("//") || previous_line.ends_with("*/") {
        return None;
    }
    Some(start)
}

/// Insert a changed type import earlier only within an already sorted, contiguous group.
/// Comments and group boundaries are barriers; existing imports never move.
pub(super) fn earlier_sorted_import_offset(
    source: &str,
    start: usize,
    end: usize,
    new_statement: &str,
) -> Option<usize> {
    if import_has_attached_comments(source, start, end) {
        return None;
    }
    let original = &source[start..end];
    let new_key = specifier_sort_key(new_statement);
    let mut next_key = specifier_sort_key(original);
    if next_key.0 != new_key.0 || new_key >= next_key {
        return None;
    }
    let analysis = crate::parser::parse_import_analysis(source).ok()?;
    let mut ranges = analysis
        .imports
        .iter()
        .map(|import| (import.statement_start, import.statement_end))
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    ranges.dedup();
    let position = ranges.iter().position(|range| *range == (start, end))?;
    let mut next_start = start;
    let mut insert_at = None;
    for &(previous_start, previous_end) in ranges[..position].iter().rev() {
        let gap = &source[previous_end..next_start];
        if !gap.chars().all(char::is_whitespace) || gap.bytes().filter(|b| *b == b'\n').count() != 1
        {
            break;
        }
        let previous_key = specifier_sort_key(&source[previous_start..previous_end]);
        if previous_key.0 != next_key.0
            || previous_key > next_key
            || new_key >= previous_key
            || import_has_attached_comments(source, previous_start, previous_end)
        {
            break;
        }
        insert_at = Some(previous_start);
        next_start = previous_start;
        next_key = previous_key;
    }
    insert_at
}

/// Where `new_statement` belongs after `from` when a later import sorts before it.
///
/// `None` means it stays immediately after the statement that produced it.
pub(super) fn later_import_insert_offset(
    source: &str,
    from: usize,
    new_statement: &str,
) -> Option<usize> {
    let new_key = specifier_sort_key(new_statement);
    let mut offset = from;
    let mut skipped_import = false;
    loop {
        let rest = source.get(offset..)?;
        let whitespace_len = rest.len() - rest.trim_start().len();
        let start = offset + whitespace_len;
        let rest = source.get(start..)?;
        if !(rest.starts_with("import ") || rest.starts_with("import{")) {
            let mut insert_at = offset;
            let bytes = source.as_bytes();
            if bytes.get(insert_at) == Some(&b'\r') {
                insert_at += 1;
            }
            if bytes.get(insert_at) == Some(&b'\n') {
                insert_at += 1;
            }
            return skipped_import.then_some(insert_at);
        }
        let end = import_statement_end(rest)?;
        if specifier_sort_key(&rest[..=end]) > new_key {
            return skipped_import.then_some(start);
        }
        skipped_import = true;
        offset = start + end + 1;
    }
}

fn comment_end_at(source: &str, index: usize) -> Option<usize> {
    let rest = source.get(index..)?;
    if rest.starts_with("//") {
        return Some(index + rest.find('\n').unwrap_or(rest.len()));
    }
    if rest.starts_with("/*") {
        return rest.find("*/").map(|offset| index + offset + 2);
    }
    None
}

/// End of a comment on the same line as an import, excluding its line ending.
pub(super) fn trailing_import_comment_end(source: &str, statement_end: usize) -> Option<usize> {
    let rest = source.get(statement_end..)?;
    let leading = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let start = statement_end + leading;
    let comment = source.get(start..)?;
    if comment.starts_with("//") {
        let end = start + comment.find('\n').unwrap_or(comment.len());
        Some(if end > start && source.as_bytes()[end - 1] == b'\r' {
            end - 1
        } else {
            end
        })
    } else if comment.starts_with("/*") {
        comment.find("*/").map(|end| start + end + 2)
    } else {
        None
    }
}

pub(super) fn after_trailing_import_comment_line(
    source: &str,
    statement_end: usize,
) -> Option<usize> {
    let end = trailing_import_comment_end(source, statement_end)?;
    let tail = &source[end..];
    if tail.starts_with("\r\n") {
        Some(end + 2)
    } else if tail.starts_with('\n') {
        Some(end + 1)
    } else {
        None
    }
}

pub(super) fn named_import_has_comments(statement: &str) -> bool {
    let Some(open) = statement.find('{') else {
        return false;
    };
    let Some(close) = statement[open..].find('}') else {
        return false;
    };
    let names = &statement[open..open + close];
    names.contains("//") || names.contains("/*")
}

pub(super) fn import_has_attached_comments(source: &str, start: usize, end: usize) -> bool {
    let preceding = source[..start]
        .trim_end_matches(['\r', '\n'])
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .trim();
    preceding.starts_with("//")
        || preceding.ends_with("*/")
        || trailing_import_comment_end(source, end).is_some()
        || named_import_has_comments(&source[start..end])
}

fn is_file_header_directive(comment: &str) -> bool {
    let text = comment
        .trim()
        .trim_start_matches("//")
        .trim_start_matches("/*")
        .trim_end_matches("*/")
        .trim();
    let text = text.to_ascii_lowercase();
    text.starts_with("eslint")
        || text.starts_with("@ts-")
        || text.starts_with("@jsx")
        || text.starts_with("@vite")
}

/// `";"` when the file terminates a statement outside braces, otherwise `""`.
///
/// Semicolons inside type literals, object literals, and strings do not count. Semicolon-free
/// TypeScript, such as the Nuxt sources in this repo's real-project checks, stays semicolon-free.
pub(super) fn statement_semicolon(source: &str) -> &'static str {
    if source_uses_semicolons(source) {
        ";"
    } else {
        ""
    }
}

fn source_uses_semicolons(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut brace = 0i32;
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                block_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            line_comment = true;
            index += 2;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            block_comment = true;
            index += 2;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
            index += 1;
            continue;
        }
        match byte {
            b'{' => brace += 1,
            b'}' => brace -= 1,
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'[' => bracket += 1,
            b']' => bracket -= 1,
            b';' if brace == 0 && paren == 0 && bracket == 0 => return true,
            _ => {}
        }
        index += 1;
    }
    false
}

/// Inclusive end of one import statement.
///
/// Semicolons inside braces, parentheses, strings, and comments do not end the statement.
/// Statements that rely on automatic semicolon insertion end at the line break after the
/// module specifier, which is how semicolon-free TypeScript writes imports.
fn import_statement_end(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut brace = 0i32;
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
                if index > 0
                    && brace == 0
                    && paren == 0
                    && bracket == 0
                    && import_clause_complete(&source[..index])
                {
                    return Some(index - 1);
                }
            }
            index += 1;
            continue;
        }
        if block_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                block_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            line_comment = true;
            index += 2;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            block_comment = true;
            index += 2;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
            index += 1;
            continue;
        }
        match byte {
            b'{' => brace += 1,
            b'}' => brace -= 1,
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'[' => bracket += 1,
            b']' => bracket -= 1,
            b';' if brace == 0 && paren == 0 && bracket == 0 => return Some(index),
            b'\n' | b'\r'
                if index > 0
                    && brace == 0
                    && paren == 0
                    && bracket == 0
                    && import_clause_complete(&source[..index]) =>
            {
                return Some(index - 1);
            }
            _ => {}
        }
        index += 1;
    }
    if !source.is_empty() && import_clause_complete(source) {
        return Some(source.len() - 1);
    }
    None
}

fn import_clause_complete(statement: &str) -> bool {
    let trimmed = statement.trim();
    if !trimmed.ends_with(['\'', '"', '}']) {
        return false;
    }
    trimmed.contains(" from ")
        || trimmed.contains(" from'")
        || trimmed.contains(" from\"")
        || is_side_effect_import(trimmed)
}

fn is_side_effect_import(statement: &str) -> bool {
    let rest = statement.trim_start().strip_prefix("import").unwrap_or("");
    let rest = rest.trim_start();
    matches!(rest.chars().next(), Some('\'' | '"'))
}

fn module_specifier_in_statement(statement: &str) -> String {
    if let Some(from) = statement.rfind(" from ") {
        let rest = statement[from + 6..].trim_start();
        if let Some(specifier) = quoted_text(rest) {
            return specifier;
        }
    }
    let trimmed = statement
        .trim_start()
        .strip_prefix("import")
        .unwrap_or(statement)
        .trim_start();
    quoted_text(trimmed).unwrap_or_else(|| trimmed.to_string())
}

fn quoted_text(source: &str) -> Option<String> {
    let quote = source.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let rest = source.get(1..)?;
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn specifier_sort_key(statement: &str) -> (u8, String) {
    let specifier = module_specifier_in_statement(statement).to_lowercase();
    (specifier_rank(&specifier), specifier)
}

fn specifier_rank(specifier: &str) -> u8 {
    if matches!(
        specifier,
        "vue" | "vue-router" | "vuex" | "nuxt" | "nuxt/app" | "#app" | "#imports"
    ) {
        0
    } else if is_relative_module_specifier(specifier) {
        2
    } else if specifier.starts_with('~') || specifier.starts_with('@') {
        3
    } else {
        1
    }
}

/// Keeps a one-name-per-line import when the original statement was wrapped and the result
/// does not fit on one line. `outside_len` is the rest of that one line, excluding the clause.
pub(super) fn format_named_import_clause(
    names: &[String],
    statement: &str,
    outside_len: usize,
) -> String {
    let single = format!("{{ {} }}", names.join(", "));
    let Some(open) = statement.find('{') else {
        return single;
    };
    let Some(close) = statement[open..].find('}') else {
        return single;
    };
    let body = &statement[open + 1..open + close];
    if !body.contains('\n')
        || names.len() <= 1
        || outside_len + single.chars().count() <= IMPORT_LINE_WIDTH
    {
        return single;
    }
    let indent = body
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let indent = line
                .chars()
                .take_while(|character| character.is_whitespace())
                .collect::<String>();
            (!indent.is_empty()).then_some(indent)
        })
        .unwrap_or_else(|| "  ".to_string());
    let close_at = open + close;
    let line_start = statement[..close_at]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let close_indent = statement[line_start..close_at]
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect::<String>();
    let trailing_comma = body.trim_end().ends_with(',');
    let rendered = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let comma = if trailing_comma || index + 1 != names.len() {
                ","
            } else {
                ""
            };
            format!("{indent}{name}{comma}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{{\n{rendered}\n{close_indent}}}")
}

const IMPORT_LINE_WIDTH: usize = 80;

/// Renders `import type` or `export type` with one name per line when one line is longer than 80
/// characters. A longer import already in the file does not raise that budget.
pub(super) fn format_named_from_statement(
    prefix: &str,
    names: &[String],
    quoted_specifier: &str,
    semicolon: &str,
    source: &str,
) -> String {
    let single = format!(
        "{prefix} {{ {} }} from {quoted_specifier}{semicolon}",
        names.join(", ")
    );
    let style = named_from_style(source);
    if names.len() <= 1 || single.chars().count() <= style.budget {
        return single;
    }
    let rendered = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let comma = if style.trailing_comma || index + 1 != names.len() {
                ","
            } else {
                ""
            };
            format!("{}{name}{comma}", style.indent)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{prefix} {{\n{rendered}\n{close}}} from {quoted_specifier}{semicolon}",
        close = style.close_indent
    )
}

struct NamedFromStyle {
    indent: String,
    close_indent: String,
    trailing_comma: bool,
    budget: usize,
}

fn is_multiline_specifier_list(line: &str) -> bool {
    if !(line.starts_with("import ")
        || line.starts_with("export ")
        || line.starts_with("import{")
        || line.starts_with("export{"))
        || !line.contains('{')
        || line.contains('}')
    {
        return false;
    }
    let before = &line[..line.find('{').unwrap_or(0)];
    !before.contains('=')
        && !before.contains("interface")
        && !before.contains("class")
        && !before.contains("enum")
}

fn named_from_style(source: &str) -> NamedFromStyle {
    let lines = source.lines().collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        if !is_multiline_specifier_list(trimmed) {
            index += 1;
            continue;
        }
        let mut name_indent = "  ".to_string();
        let mut last_had_comma = false;
        index += 1;
        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim();
            if trimmed.starts_with('}') {
                let close_indent = line
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .collect();
                return NamedFromStyle {
                    indent: name_indent,
                    close_indent,
                    trailing_comma: last_had_comma,
                    budget: IMPORT_LINE_WIDTH,
                };
            }
            if !trimmed.is_empty() {
                let indent = line
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .collect::<String>();
                if !indent.is_empty() {
                    name_indent = indent;
                }
                last_had_comma = trimmed.ends_with(',');
            }
            index += 1;
        }
    }
    // No multiline import or export list shows the choice. A type body is not that list.
    // Prettier still writes a trailing comma on a wrapped specifier list.
    NamedFromStyle {
        indent: "  ".to_string(),
        close_indent: String::new(),
        trailing_comma: true,
        budget: IMPORT_LINE_WIDTH,
    }
}
