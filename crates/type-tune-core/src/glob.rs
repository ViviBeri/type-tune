//! Minimal project-relative glob matching used for referrer exclusions.

/// Returns whether a POSIX path matches a `*`, `**`, and `?` glob.
#[must_use]
pub fn matches(path: &str, pattern: &str) -> bool {
    let path: Vec<char> = path.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    // A row records matches for one pattern suffix; at most three later rows are needed.
    let mut matched = vec![vec![false; path.len() + 1]; 4];
    matched[pattern.len() % 4][path.len()] = true;
    for pattern_index in (0..pattern.len()).rev() {
        let row = pattern_index % 4;
        let segment_start = pattern_index == 0 || pattern[pattern_index - 1] == '/';
        if segment_start && pattern[pattern_index..].starts_with(&['*', '*', '/']) {
            let mut later_directory_matches = false;
            for path_index in (0..=path.len()).rev() {
                if path_index < path.len() && path[path_index] == '/' {
                    later_directory_matches |= matched[row][path_index + 1];
                }
                matched[row][path_index] =
                    matched[(pattern_index + 3) % 4][path_index] || later_directory_matches;
            }
        } else if segment_start && pattern[pattern_index..] == ['*', '*'] {
            for path_index in (0..=path.len()).rev() {
                matched[row][path_index] = matched[(pattern_index + 2) % 4][path_index]
                    || (path_index < path.len() && matched[row][path_index + 1]);
            }
        } else {
            for path_index in (0..=path.len()).rev() {
                matched[row][path_index] = match pattern[pattern_index] {
                    '*' => {
                        matched[(pattern_index + 1) % 4][path_index]
                            || (path_index < path.len()
                                && path[path_index] != '/'
                                && matched[row][path_index + 1])
                    }
                    character => {
                        path_index < path.len()
                            && (character == path[path_index]
                                || (character == '?' && path[path_index] != '/'))
                            && matched[(pattern_index + 1) % 4][path_index + 1]
                    }
                };
            }
        }
    }
    matched[0][0]
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn supports_the_documented_wildcards() {
        assert!(matches("src/order.test.ts", "**/*.test.ts"));
        assert!(matches("order.test.ts", "**/*.test.ts"));
        assert!(matches("src/story.tsx", "src/stor?.tsx"));
        assert!(!matches("src/nested/order.test.ts", "src/*.test.ts"));
    }

    #[test]
    fn handles_repeated_wildcards_and_nested_directories() {
        assert!(matches("a/b/c/file.ts", "**/b/**/file.ts"));
        assert!(!matches("a/b/c/file.js", "**/b/**/file.ts"));
        assert!(!matches(&"a".repeat(100), "*a*a*a*a*a*a*a*a*a*b"));
        assert!(matches("src/nested/file.ts", "src/**"));
        assert!(matches("src/file.ts", "src/**.ts"));
        assert!(!matches("src/nested/file.ts", "src/**.ts"));
        assert!(!matches("src/nested/file.js", "src/**.ts"));
        assert!(!matches("src/a/b/file.ts", "src/a**/file.ts"));
    }

    #[test]
    fn question_mark_matches_one_unicode_character() {
        assert!(matches("src/café.ts", "src/caf?.ts"));
        assert!(!matches("src/caféé.ts", "src/caf?.ts"));
        assert!(!matches("src/caf/é.ts", "src/caf?.ts"));
    }
}
