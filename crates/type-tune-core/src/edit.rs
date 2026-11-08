//! Safe deterministic text edit application.

use std::collections::HashSet;

use crate::{Result, TypeTuneError};

/// One replacement expressed as byte offsets in a UTF-8 source string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Inclusive byte offset where replacement begins.
    pub start: usize,
    /// Exclusive byte offset where replacement ends.
    pub end: usize,
    /// Replacement text.
    pub replacement: String,
}

/// Applies non-overlapping text edits using offsets in the original source.
///
/// # Errors
///
/// Returns an error for invalid UTF-8 boundaries, out-of-range edits, or overlapping ranges.
pub fn apply_text_edits(source: &str, edits: &[TextEdit]) -> Result<String> {
    let mut edits: Vec<_> = edits.iter().collect();
    edits.sort_by_key(|edit| (edit.start, edit.end));
    let mut seen = HashSet::new();
    edits.retain(|edit| seen.insert((edit.start, edit.end, edit.replacement.as_str())));
    let mut previous_end = 0;
    for edit in &edits {
        if edit.start > edit.end
            || edit.end > source.len()
            || !source.is_char_boundary(edit.start)
            || !source.is_char_boundary(edit.end)
        {
            return Err(TypeTuneError::new(format!(
                "Invalid edit range {}..{} for source of {} bytes.",
                edit.start,
                edit.end,
                source.len()
            )));
        }
        if edit.start < previous_end {
            return Err(TypeTuneError::new(format!(
                "Overlapping edit ranges near {}..{}.",
                edit.start, edit.end
            )));
        }
        previous_end = edit.end;
    }
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for edit in edits {
        output.push_str(&source[cursor..edit.start]);
        output.push_str(&edit.replacement);
        cursor = edit.end;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{TextEdit, apply_text_edits};

    #[test]
    fn applies_edits_using_original_offsets() {
        let output = apply_text_edits(
            "export type Shared = string;",
            &[
                TextEdit {
                    start: 0,
                    end: 11,
                    replacement: "export interface".into(),
                },
                TextEdit {
                    start: 21,
                    end: 27,
                    replacement: "number".into(),
                },
            ],
        )
        .expect("applies");
        assert_eq!(output, "export interface Shared = number;");
    }

    #[test]
    fn rejects_overlapping_edits() {
        assert!(
            apply_text_edits(
                "source",
                &[
                    TextEdit {
                        start: 0,
                        end: 3,
                        replacement: String::new(),
                    },
                    TextEdit {
                        start: 2,
                        end: 4,
                        replacement: String::new(),
                    },
                ],
            )
            .is_err()
        );
    }

    #[test]
    fn applies_duplicate_edits_once() {
        let edit = TextEdit {
            start: 0,
            end: 6,
            replacement: "output".into(),
        };
        let output = apply_text_edits("source", &[edit.clone(), edit]).expect("applies");
        assert_eq!(output, "output");
    }

    #[test]
    fn deduplicates_interleaved_insertions_and_preserves_order() {
        let first = TextEdit {
            start: 1,
            end: 1,
            replacement: "A".into(),
        };
        let second = TextEdit {
            replacement: "B".into(),
            ..first.clone()
        };
        assert_eq!(
            apply_text_edits("xy", &[first.clone(), second, first]).expect("applies"),
            "xABy"
        );
    }

    #[test]
    fn validates_utf8_boundaries() {
        let edit = TextEdit {
            start: 1,
            end: 2,
            replacement: String::new(),
        };
        assert!(apply_text_edits("é", &[edit]).is_err());
    }
}
