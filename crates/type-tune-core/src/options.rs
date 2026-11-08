//! Shared native CLI option validation.

use crate::{Result, TypeTuneError};

/// Default minimum unique referrer count for scan and plan when the flag is omitted.
pub const DEFAULT_MIN_REFERRERS: usize = 2;

/// Parses a strictly positive integer option.
///
/// # Errors
///
/// Returns an error for non-integer or non-positive values.
pub fn parse_positive_integer(value: &str, option_name: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            TypeTuneError::new(format!(
                "Invalid {option_name} \"{value}\". Expected a positive integer."
            ))
        })
}

/// Parses comma-separated project-relative POSIX glob patterns.
///
/// # Errors
///
/// Returns an error for empty, absolute, drive-prefixed, traversal, NUL, or backslash patterns.
pub fn parse_project_globs(value: &str, option_name: &str) -> Result<Vec<String>> {
    let globs = value
        .split(',')
        .map(str::trim)
        .map(String::from)
        .collect::<Vec<_>>();
    if globs.iter().any(|glob| {
        glob.is_empty()
            || glob.contains('\0')
            || glob.starts_with('/')
            || glob.contains('\\')
            || (glob.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && glob.as_bytes().get(1) == Some(&b':'))
            || glob.split('/').any(|segment| matches!(segment, "." | ".."))
    }) {
        return Err(TypeTuneError::new(format!(
            "Invalid {option_name} \"{value}\". Expected comma-separated project-relative POSIX globs."
        )));
    }
    Ok(globs)
}

/// Parses the CLI declaration kind aliases into native candidate kinds.
///
/// # Errors
///
/// Returns an error when a kind other than `type` or `interface` is provided.
pub fn parse_include_kinds(value: &str) -> Result<Vec<String>> {
    value
        .split(',')
        .map(str::trim)
        .map(|kind| match kind {
            "type" => Ok("typeAlias".into()),
            "interface" => Ok("interface".into()),
            _ => Err(TypeTuneError::new(format!(
                "Invalid --include \"{value}\". Expected \"type\" or \"interface\"."
            ))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MIN_REFERRERS, parse_include_kinds, parse_positive_integer, parse_project_globs,
    };

    #[test]
    fn defaults_minimum_referrers_to_two() {
        assert_eq!(DEFAULT_MIN_REFERRERS, 2);
    }

    #[test]
    fn rejects_non_positive_values() {
        assert_eq!(parse_positive_integer("2", "--min-referrers"), Ok(2));
        assert!(parse_positive_integer("0", "--min-referrers").is_err());
        assert!(parse_positive_integer("x", "--min-referrers").is_err());
    }

    #[test]
    fn parses_only_safe_project_globs() {
        assert_eq!(
            parse_project_globs("**/*.test.ts,src/**", "--referrer-count-exclude"),
            Ok(vec!["**/*.test.ts".into(), "src/**".into()])
        );
        assert!(parse_project_globs("../test/**", "--referrer-count-exclude").is_err());
        assert!(parse_project_globs("C:/outside/**", "--referrer-count-exclude").is_err());
        assert!(parse_project_globs("src/\0test.ts", "--referrer-count-exclude").is_err());
    }

    #[test]
    fn parses_cli_declaration_kind_aliases() {
        assert_eq!(
            parse_include_kinds("type,interface"),
            Ok(vec!["typeAlias".into(), "interface".into()])
        );
        assert!(parse_include_kinds("enum").is_err());
    }
}
