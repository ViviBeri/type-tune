//! Deterministic destination resolution for extracted declarations.

/// Destination placement strategy for extracted declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlacementStyle {
    /// Write into a `types` directory beside the source.
    Collocation,
    /// Write beneath repository-level `types/`.
    RepoTypes,
}

/// Resolves a collocated destination path for a project-relative source path.
#[must_use]
pub fn resolve_collocated_destination(source_path: &str) -> String {
    let mut segments = source_path.split('/').collect::<Vec<_>>();
    let source_name = segments.pop().unwrap_or("types.ts");
    let source_base = source_name
        .rsplit_once('.')
        .map_or(source_name, |(base, _)| base);
    let directory = segments.last().copied().unwrap_or("");
    let domain = if directory.is_empty() || directory == "types" {
        source_base
    } else {
        directory
    };
    let filename = format!("{}.ts", kebab_case(domain));
    if segments.last().is_none_or(|value| *value != "types") {
        segments.push("types");
    }
    segments.push(&filename);
    segments.join("/")
}

/// Resolves a repository-wide `types/` destination path.
#[must_use]
pub fn resolve_repo_types_destination(source_path: &str) -> String {
    let collocated = resolve_collocated_destination(source_path);
    if collocated.starts_with("types/") {
        collocated
    } else {
        format!("types/{}", collocated)
    }
}

/// Resolves a destination using the requested placement strategy.
#[must_use]
pub fn resolve_destination(source_path: &str, placement_style: PlacementStyle) -> String {
    match placement_style {
        PlacementStyle::Collocation => resolve_collocated_destination(source_path),
        PlacementStyle::RepoTypes => resolve_repo_types_destination(source_path),
    }
}

fn kebab_case(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            output.push('-');
        }
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    output.trim_matches('-').into()
}

#[cfg(test)]
mod tests {
    use super::{resolve_collocated_destination, resolve_repo_types_destination};

    #[test]
    fn resolves_collocated_destination_paths() {
        assert_eq!(
            resolve_collocated_destination("src/feature/OrderSchema.ts"),
            "src/feature/types/feature.ts"
        );
        assert_eq!(
            resolve_collocated_destination("src/types/shared.ts"),
            "src/types/shared.ts"
        );
        assert_eq!(
            resolve_collocated_destination("types/shared.ts"),
            "types/shared.ts"
        );
        assert_eq!(
            resolve_repo_types_destination("src/feature/OrderSchema.ts"),
            "types/src/feature/types/feature.ts"
        );
        assert_eq!(
            resolve_repo_types_destination("types/shared.ts"),
            "types/shared.ts"
        );
    }
}
