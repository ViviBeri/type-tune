//! Deterministic relative module specifier generation.

pub(crate) fn is_relative_module_specifier(specifier: &str) -> bool {
    matches!(specifier, "." | "..") || specifier.starts_with("./") || specifier.starts_with("../")
}

/// Creates a relative TypeScript module specifier from a referrer to a destination file.
#[must_use]
pub fn relative_module_specifier(referrer_path: &str, destination_path: &str) -> String {
    relative_module_specifier_with_extension(referrer_path, destination_path, "")
}

/// Creates a relative TypeScript module specifier and appends a JavaScript-like extension.
#[must_use]
pub fn relative_module_specifier_with_extension(
    referrer_path: &str,
    destination_path: &str,
    extension: &str,
) -> String {
    let mut from = referrer_path.split('/').collect::<Vec<_>>();
    from.pop();
    let mut to = destination_path.split('/').collect::<Vec<_>>();
    let file_name = to.pop().unwrap_or("");
    let base = file_name
        .rsplit_once('.')
        .map_or(file_name, |(base, _)| base);
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let parent_count = from.len() - common;
    let mut parts = vec![".."; parent_count];
    parts.extend_from_slice(&to[common..]);
    parts.push(base);
    let value = parts.join("/");
    let specifier = if parent_count > 0 {
        value
    } else {
        format!("./{value}")
    };
    format!("{specifier}{extension}")
}

/// Returns a JavaScript-like extension used by a module specifier.
#[must_use]
pub fn js_like_module_specifier_extension(specifier: &str) -> Option<&str> {
    [".cjs", ".mjs", ".js"]
        .into_iter()
        .find(|extension| specifier.ends_with(*extension))
}

/// Returns the extension a rewritten specifier should keep.
///
/// TypeScript source extensions stay in place. A project that imports `./session.ts` cannot
/// resolve the same file through `./session.js` under `moduleResolution: node`.
#[must_use]
pub fn preserved_module_specifier_extension(specifier: &str) -> Option<&str> {
    [".mjs", ".cjs", ".mts", ".cts", ".tsx", ".jsx", ".js", ".ts"]
        .into_iter()
        .find(|extension| specifier.ends_with(*extension))
}

#[cfg(test)]
mod tests {
    use super::{
        js_like_module_specifier_extension, preserved_module_specifier_extension,
        relative_module_specifier, relative_module_specifier_with_extension,
    };

    #[test]
    fn creates_relative_specifiers() {
        assert_eq!(
            relative_module_specifier("src/feature/view.ts", "src/feature/types/feature.ts"),
            "./types/feature"
        );
        assert_eq!(
            relative_module_specifier("src/app/view.ts", "types/src/feature/types/feature.ts"),
            "../../types/src/feature/types/feature"
        );
        assert_eq!(
            relative_module_specifier("src/view.ts", "src/..hidden.ts"),
            "./..hidden"
        );
    }

    #[test]
    fn preserves_javascript_module_extensions() {
        assert_eq!(
            relative_module_specifier_with_extension(
                "src/app/view.ts",
                "src/types/shared.ts",
                ".js"
            ),
            "../types/shared.js"
        );
        assert_eq!(
            js_like_module_specifier_extension("./shared.mjs"),
            Some(".mjs")
        );
        assert_eq!(js_like_module_specifier_extension("./shared.ts"), None);
        assert_eq!(
            preserved_module_specifier_extension("./shared.ts"),
            Some(".ts")
        );
        assert_eq!(
            preserved_module_specifier_extension("./shared.tsx"),
            Some(".tsx")
        );
        assert_eq!(preserved_module_specifier_extension("./shared"), None);
    }
}
