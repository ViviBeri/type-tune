//! Normalize TypeScript configuration JSONC for JSON parsing.

use crate::{Result, TypeTuneError};

pub(super) fn normalize_jsonc(content: &str) -> Result<String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let without_comments = strip_jsonc_comments(content)?;
    Ok(strip_trailing_commas(&without_comments))
}

fn strip_jsonc_comments(content: &str) -> Result<String> {
    let mut characters = content.chars().peekable();
    let mut output = String::with_capacity(content.len());
    let mut in_string = false;
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            output.push(character);
            continue;
        }
        if character == '/' && characters.peek() == Some(&'/') {
            output.push(' ');
            characters.next();
            for character in characters.by_ref() {
                if matches!(character, '\r' | '\n') {
                    output.push(character);
                    break;
                }
            }
            continue;
        }
        if character == '/' && characters.peek() == Some(&'*') {
            output.push(' ');
            characters.next();
            let mut terminated = false;
            while let Some(character) = characters.next() {
                if character == '*' && characters.peek() == Some(&'/') {
                    characters.next();
                    terminated = true;
                    break;
                }
                if matches!(character, '\r' | '\n') {
                    output.push(character);
                }
            }
            if !terminated {
                return Err(TypeTuneError::new(
                    "Unterminated block comment in tsconfig JSONC.",
                ));
            }
            continue;
        }
        output.push(character);
    }
    Ok(output)
}

fn strip_trailing_commas(content: &str) -> String {
    let mut characters = content.chars();
    let mut output = String::with_capacity(content.len());
    let mut in_string = false;
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            output.push(character);
            continue;
        }
        if character == ','
            && characters
                .clone()
                .find(|next| !next.is_whitespace())
                .is_some_and(|next| matches!(next, ']' | '}'))
        {
            continue;
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::normalize_jsonc;

    #[test]
    fn preserves_strings_and_newlines_while_removing_jsonc_syntax() {
        let input = "{\n  \"url\": \"https://example.test/é,}\\\"\", // comment\n  \"items\": [1, /* comment\n  continued */ 2,],\n}";
        let normalized = normalize_jsonc(input).expect("normalizes");
        let value: serde_json::Value = serde_json::from_str(&normalized).expect("parses");
        assert_eq!(value["url"], "https://example.test/é,}\"");
        assert_eq!(value["items"], serde_json::json!([1, 2]));
        assert_eq!(normalized.lines().count(), input.lines().count());
    }

    #[test]
    fn rejects_unterminated_block_comments() {
        assert!(normalize_jsonc("{/* unfinished").is_err());
    }

    #[test]
    fn accepts_a_leading_byte_order_mark() {
        let normalized =
            normalize_jsonc("\u{feff}{\"value\":\"\u{feff}inside\"}").expect("normalizes");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&normalized).expect("parses")["value"],
            "\u{feff}inside"
        );
    }

    #[test]
    fn preserves_carriage_return_line_endings_in_comments() {
        for input in [
            "{\r// comment\r\"value\": 1\r}",
            "{\r\n// comment\r\n\"value\": 1\r\n}",
            "{\r/* comment\rcontinued */\"value\": 1\r}",
        ] {
            let normalized = normalize_jsonc(input).expect("normalizes");
            assert_eq!(
                normalized.matches('\r').count(),
                input.matches('\r').count()
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&normalized).expect("parses")["value"],
                1
            );
        }
    }

    #[test]
    fn comments_do_not_join_adjacent_tokens() {
        let normalized = normalize_jsonc("{\"value\": 1/* comment */2}").expect("normalizes");
        assert!(serde_json::from_str::<serde_json::Value>(&normalized).is_err());
        let normalized = normalize_jsonc("{\"value\": 1/* comment */}").expect("normalizes");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&normalized).expect("parses")["value"],
            1
        );
    }
}
