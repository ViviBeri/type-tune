//! Interactive plan markdown review helpers shared by the native CLI.

use std::collections::BTreeSet;

use crate::{Result, TypeTuneError};

/// Zero-based page size for the interactive review list.
pub const REVIEW_PAGE_SIZE: usize = 10;

/// One reviewable candidate row parsed from plan markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPlanItem {
    /// Zero-based markdown line index.
    pub line_index: usize,
    /// Candidate id.
    pub candidate_id: String,
    /// Candidate symbol name.
    pub symbol_name: String,
    /// Current review status (`apply` or `ignore`).
    pub status: String,
    /// Source path when present in plan details.
    pub source_path: Option<String>,
    /// Destination path when present in plan details.
    pub destination_path: Option<String>,
}

/// Parsed plan markdown ready for interactive review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPlanDocument {
    /// Original markdown lines.
    pub lines: Vec<String>,
    /// Reviewable candidate items.
    pub items: Vec<ReviewPlanItem>,
}

/// Mutable interactive review UI state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewState {
    /// Parsed plan markdown document.
    pub document: ReviewPlanDocument,
    /// Currently selected item index.
    pub selected_index: usize,
    /// Whether the previous key was Enter.
    pub previous_key_was_enter: bool,
    /// Whether the user saved changes.
    pub saved: bool,
}

/// Whether the interactive review loop should continue or exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewKeyAction {
    /// Keep the review UI open.
    Continue,
    /// Exit the review UI.
    Exit,
}

/// Parses reviewable candidate items from plan markdown.
///
/// # Errors
///
/// Returns an error for duplicate candidate ids or invalid status markers.
pub fn parse_review_plan_markdown(content: &str) -> Result<ReviewPlanDocument> {
    let lines = split_markdown_lines(content);
    let mut items = Vec::new();
    let mut seen_ids = BTreeSet::new();

    for (line_index, line) in lines.iter().enumerate() {
        let Some(candidate_line) = parse_candidate_line(line)? else {
            continue;
        };
        let Some(symbol_name) = candidate_line.symbol_name else {
            continue;
        };
        if !seen_ids.insert(candidate_line.candidate_id.clone()) {
            return Err(TypeTuneError::new(format!(
                "Duplicate candidate id \"{}\" in plan markdown.",
                candidate_line.candidate_id
            )));
        }
        let details = read_plan_item_details(&lines, line_index);
        items.push(ReviewPlanItem {
            line_index,
            candidate_id: candidate_line.candidate_id,
            symbol_name,
            status: candidate_line.status,
            source_path: details.0,
            destination_path: details.1,
        });
    }

    Ok(ReviewPlanDocument { lines, items })
}

/// Renders reviewed candidate statuses back into plan markdown.
///
/// # Errors
///
/// Returns an error when a candidate line is missing or no longer parseable.
pub fn render_reviewed_plan_markdown(document: &ReviewPlanDocument) -> Result<String> {
    let mut lines = document.lines.clone();
    for item in &document.items {
        let line = lines.get(item.line_index).ok_or_else(|| {
            TypeTuneError::new(format!(
                "Plan markdown line missing for candidate {}.",
                item.candidate_id
            ))
        })?;
        lines[item.line_index] = replace_candidate_line_status(line, &item.status)?;
        update_status_detail_line(&mut lines, item);
    }
    Ok(format!("{}\n", lines.join("\n").trim_end_matches('\n')))
}

/// Toggles one review item between apply and ignore.
pub fn toggle_review_plan_item(item: &mut ReviewPlanItem) {
    item.status = if item.status == "apply" {
        "ignore".into()
    } else {
        "apply".into()
    };
}

/// Handles one key press for the review UI.
pub fn handle_review_key(state: &mut ReviewState, key: &str) -> ReviewKeyAction {
    match key {
        "\u{0003}" | "\u{001b}" => {
            state.previous_key_was_enter = false;
            ReviewKeyAction::Exit
        }
        " " => {
            if let Some(item) = state.document.items.get_mut(state.selected_index) {
                toggle_review_plan_item(item);
            }
            state.previous_key_was_enter = false;
            state.saved = false;
            ReviewKeyAction::Continue
        }
        "\t" | "\u{001b}[B" | "j" => {
            move_review_selection(state, 1);
            state.previous_key_was_enter = false;
            ReviewKeyAction::Continue
        }
        "\u{001b}[Z" | "\u{001b}[A" | "k" => {
            move_review_selection(state, -1);
            state.previous_key_was_enter = false;
            ReviewKeyAction::Continue
        }
        "\u{0004}" | "\u{001b}[6~" => {
            move_review_selection(state, REVIEW_PAGE_SIZE as isize);
            state.previous_key_was_enter = false;
            ReviewKeyAction::Continue
        }
        "\u{0015}" | "\u{001b}[5~" => {
            move_review_selection(state, -(REVIEW_PAGE_SIZE as isize));
            state.previous_key_was_enter = false;
            ReviewKeyAction::Continue
        }
        "\r" | "\n" => {
            if state.previous_key_was_enter {
                state.saved = true;
                ReviewKeyAction::Exit
            } else {
                state.previous_key_was_enter = true;
                state.saved = true;
                ReviewKeyAction::Continue
            }
        }
        _ => {
            state.previous_key_was_enter = false;
            ReviewKeyAction::Continue
        }
    }
}

/// Renders the interactive review screen text without clearing the terminal.
#[must_use]
pub fn render_review_screen(state: &ReviewState) -> String {
    let mut output = String::from("type-tune review\n\n");
    output.push_str(
        "Space: toggle apply/ignore  j/Down: next  k/Up: previous  Ctrl+D/PageDown: +10  Ctrl+U/PageUp: -10  Enter: save  Enter twice: save and exit  Esc: quit\n\n",
    );
    let (start_index, end_index) = visible_review_range(state);
    output.push_str(&format!(
        "Candidates {}-{} of {}\n\n",
        start_index + 1,
        end_index,
        state.document.items.len()
    ));
    for index in start_index..end_index {
        let Some(item) = state.document.items.get(index) else {
            continue;
        };
        let prefix = if index == state.selected_index {
            "> "
        } else {
            "  "
        };
        output.push_str(prefix);
        output.push_str(&render_review_item(item));
        output.push('\n');
    }
    output.push('\n');
    output.push_str(if state.saved {
        "Saved. Press Enter again to exit.\n"
    } else {
        "Unsaved changes.\n"
    });
    output
}

fn visible_review_range(state: &ReviewState) -> (usize, usize) {
    let start_index = (state.selected_index / REVIEW_PAGE_SIZE) * REVIEW_PAGE_SIZE;
    let end_index = (start_index + REVIEW_PAGE_SIZE).min(state.document.items.len());
    (start_index, end_index)
}

fn move_review_selection(state: &mut ReviewState, offset: isize) {
    if state.document.items.is_empty() {
        state.selected_index = 0;
        return;
    }
    let next = state.selected_index as isize + offset;
    state.selected_index = next.clamp(0, (state.document.items.len() - 1) as isize) as usize;
}

fn render_review_item(item: &ReviewPlanItem) -> String {
    let status = if item.status == "apply" {
        "apply "
    } else {
        "ignore"
    };
    let path_summary = match (&item.source_path, &item.destination_path) {
        (Some(source), Some(destination)) => format!(" {source} -> {destination}"),
        _ => String::new(),
    };
    format!("[{status}] {}{path_summary}", item.symbol_name)
}

#[derive(Debug)]
struct ParsedCandidateLine {
    candidate_id: String,
    symbol_name: Option<String>,
    status: String,
    format: CandidateLineFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateLineFormat {
    Explicit,
    Legacy,
}

fn parse_candidate_line(line: &str) -> Result<Option<ParsedCandidateLine>> {
    let Some(rest) = line.strip_prefix("- [") else {
        return Ok(None);
    };
    let Some((marker, remainder)) = rest.split_once(']') else {
        return Ok(None);
    };
    let remainder = remainder.strip_prefix(' ').unwrap_or(remainder);
    let status = match marker {
        "apply" | " " => "apply".to_owned(),
        "ignore" | "x" | "X" => "ignore".to_owned(),
        _ => return Ok(None),
    };
    let format = if matches!(marker, "apply" | "ignore") {
        CandidateLineFormat::Explicit
    } else {
        CandidateLineFormat::Legacy
    };
    let mut parts = remainder.splitn(2, ' ');
    let candidate_id = parts.next().unwrap_or_default().trim();
    if candidate_id.is_empty() {
        return Err(TypeTuneError::new("Missing candidate id in plan markdown."));
    }
    let symbol_name = parts
        .next()
        .and_then(|value| value.trim().strip_prefix('`'))
        .and_then(|value| value.strip_suffix('`'))
        .map(str::to_owned);
    Ok(Some(ParsedCandidateLine {
        candidate_id: candidate_id.to_owned(),
        symbol_name,
        status,
        format,
    }))
}

fn replace_candidate_line_status(line: &str, status: &str) -> Result<String> {
    let parsed = parse_candidate_line(line)?.ok_or_else(|| {
        TypeTuneError::new(format!("Invalid plan markdown candidate line \"{line}\"."))
    })?;
    let _ = parsed.format;
    Ok(format!(
        "- [{status}] {}{}",
        parsed.candidate_id,
        parsed
            .symbol_name
            .map(|name| format!(" `{name}`"))
            .unwrap_or_default()
    ))
}

fn read_plan_item_details(
    lines: &[String],
    start_index: usize,
) -> (Option<String>, Option<String>) {
    let mut source_path = None;
    let mut destination_path = None;
    for line in lines.iter().skip(start_index + 1) {
        if parse_candidate_line(line).ok().flatten().is_some() {
            break;
        }
        if let Some(value) = line.trim_start().strip_prefix("- from: `")
            && let Some(value) = value.strip_suffix('`')
        {
            source_path = Some(value.to_owned());
        }
        if let Some(value) = line.trim_start().strip_prefix("- to: `")
            && let Some(value) = value.strip_suffix('`')
        {
            destination_path = Some(value.to_owned());
        }
    }
    (source_path, destination_path)
}

fn update_status_detail_line(lines: &mut [String], item: &ReviewPlanItem) {
    for line in lines.iter_mut().skip(item.line_index + 1) {
        if parse_candidate_line(line).ok().flatten().is_some() {
            return;
        }
        if line.trim_start().starts_with("- status: `") {
            *line = format!("  - status: `{}`", item.status);
            return;
        }
    }
}

fn split_markdown_lines(content: &str) -> Vec<String> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.is_empty() {
        return Vec::new();
    }
    normalized.split('\n').map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ReviewKeyAction, ReviewState, handle_review_key, parse_review_plan_markdown,
        render_reviewed_plan_markdown, toggle_review_plan_item,
    };

    #[test]
    fn parses_review_items_and_detail_paths() {
        let document = parse_review_plan_markdown(sample_markdown()).expect("parses");
        assert_eq!(document.items.len(), 2);
        assert_eq!(document.items[0].candidate_id, "abc123");
        assert_eq!(document.items[0].symbol_name, "OrderViewModel");
        assert_eq!(document.items[0].status, "apply");
        assert_eq!(
            document.items[0].source_path.as_deref(),
            Some("src/order.ts")
        );
        assert_eq!(
            document.items[0].destination_path.as_deref(),
            Some("src/types/order.ts")
        );
        assert_eq!(document.items[1].status, "ignore");
    }

    #[test]
    fn toggles_items_and_renders_explicit_statuses() {
        let mut document = parse_review_plan_markdown(sample_markdown()).expect("parses");
        toggle_review_plan_item(&mut document.items[0]);
        toggle_review_plan_item(&mut document.items[1]);
        let rendered = render_reviewed_plan_markdown(&document).expect("renders");
        assert!(rendered.contains("- [ignore] abc123 `OrderViewModel`"));
        assert!(rendered.contains("- [apply] def456 `BillingSummary`"));
        assert!(rendered.contains("  - status: `ignore`"));
        assert!(rendered.contains("  - status: `apply`"));
    }

    #[test]
    fn accepts_legacy_checkbox_markdown() {
        let document = parse_review_plan_markdown(legacy_markdown()).expect("parses");
        assert_eq!(document.items[0].status, "apply");
        assert_eq!(document.items[1].status, "ignore");
        let rendered = render_reviewed_plan_markdown(&document).expect("renders");
        assert!(rendered.contains("- [apply] abc123 `OrderViewModel`"));
        assert!(rendered.contains("- [ignore] def456 `BillingSummary`"));
    }

    #[test]
    fn exits_after_enter_twice() {
        let document = parse_review_plan_markdown(sample_markdown()).expect("parses");
        let mut state = ReviewState {
            document,
            selected_index: 0,
            previous_key_was_enter: false,
            saved: false,
        };
        assert_eq!(
            handle_review_key(&mut state, "\r"),
            ReviewKeyAction::Continue
        );
        assert!(state.saved);
        assert_eq!(handle_review_key(&mut state, "\r"), ReviewKeyAction::Exit);
    }

    fn sample_markdown() -> &'static str {
        "# type-tune plan\n\nProject: `tsconfig.json`\nCreated: `2026-05-12T00:00:00.000Z`\nCandidates: 2\n\n## Candidates\n\n- [apply] abc123 `OrderViewModel`\n  - status: `apply`\n  - kind: `typeAlias`\n  - from: `src/order.ts`\n  - to: `src/types/order.ts`\n  - referrers: 2\n  - local dependencies: `none`\n  - strategies:\n    - `src/a.ts`: `relative` import `./types/order`\n\n- [ignore] def456 `BillingSummary`\n  - status: `ignore`\n  - kind: `interface`\n  - from: `src/billing.ts`\n  - to: `src/types/billing.ts`\n  - referrers: 3\n  - local dependencies: `none`\n  - strategies:\n    - `src/b.ts`: `relative` import `./types/billing`\n"
    }

    fn legacy_markdown() -> &'static str {
        "# type-tune plan\n\n## Candidates\n\n- [ ] abc123 `OrderViewModel`\n  - from: `src/order.ts`\n  - to: `src/types/order.ts`\n\n- [x] def456 `BillingSummary`\n  - from: `src/billing.ts`\n  - to: `src/types/billing.ts`\n"
    }
}
