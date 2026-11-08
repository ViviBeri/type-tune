//! Reviewable native plan generation.

mod markdown;
mod validation;

pub use markdown::{merge_plan_markdown_state, render_plan_markdown};
pub use validation::{
    read_plan_json, validate_plan_candidate_metadata, validate_plan_candidate_path_boundaries,
    validate_plan_freshness,
};

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    Result, TypeTuneError, destination::PlacementStyle, hash::sha256, model::Candidate,
    parser::parse_type_declarations,
};

/// Effective native CLI options persisted in a plan file.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOptions {
    /// Path to tsconfig as provided by the CLI.
    pub project_path: String,
    /// Minimum number of unique referrers required for a candidate.
    pub min_referrers: usize,
    /// Project-relative globs excluded from the eligibility count.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referrer_count_exclude_globs: Vec<String>,
    /// Destination placement strategy.
    pub placement_style: PlacementStyle,
    /// Declaration kinds included in the scan.
    pub include_kinds: Vec<String>,
    /// Initial ignore entries.
    pub ignore_entries: Vec<String>,
    /// Initial status for entries not matched by ignore entries.
    #[serde(default = "default_ignore_status")]
    pub default_status: String,
    /// Whether declarations in `types` directories or `types.ts` files were included.
    #[serde(default)]
    pub include_existing_types: bool,
    /// Whether generated source files were included.
    #[serde(default)]
    pub include_generated: bool,
}

/// One reviewable native plan entry.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEntry {
    /// Fully resolved scan candidate.
    pub candidate: Candidate,
    /// Hash of the source declaration text and path.
    pub source_fingerprint: String,
    /// Whether native apply should execute or skip this entry.
    pub status: String,
    /// Destination edited in the review Markdown, never trusted from JSON.
    #[serde(skip)]
    pub reviewed_destination: Option<String>,
    /// Optional provider review annotation created during AI-assisted planning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_review: Option<crate::ai::AiReviewDecision>,
}

/// JSON plan format shared by native planning and applying.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFile {
    /// Plan format version.
    pub version: u8,
    /// Root project or tsconfig path.
    pub project_path: String,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
    /// Effective options used to generate this plan.
    pub options: PlanOptions,
    /// Candidate entries in deterministic order.
    pub entries: Vec<PlanEntry>,
}

/// Builds a reviewable plan from candidates and their source contents.
///
/// # Errors
///
/// Returns an error for an invalid status, duplicate candidate ID, missing source, or declaration.
pub fn build_plan(
    project_path: String,
    candidates: Vec<Candidate>,
    sources: &BTreeMap<String, String>,
    options: PlanOptions,
) -> Result<PlanFile> {
    if !matches!(options.default_status.as_str(), "apply" | "ignore") {
        return Err(TypeTuneError::new(
            "Invalid default plan status. Expected \"apply\" or \"ignore\".",
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &candidates {
        if !ids.insert(candidate.id.as_str()) {
            return Err(TypeTuneError::new(format!(
                "Duplicate candidate id \"{}\" in plan entries.",
                candidate.id
            )));
        }
    }
    let mut entries = candidates
        .into_iter()
        .map(|candidate| {
            let source = sources.get(&candidate.source_path).ok_or_else(|| {
                TypeTuneError::new(format!(
                    "Could not find source for {}.",
                    candidate.source_path
                ))
            })?;
            let source_fingerprint = create_source_fingerprint(&candidate, source)?;
            let status = if candidate.is_ignored(&options.ignore_entries) {
                "ignore".into()
            } else {
                options.default_status.clone()
            };
            Ok(PlanEntry {
                candidate,
                source_fingerprint,
                status,
                reviewed_destination: None,
                ai_review: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by(|left, right| left.candidate.stable_key.cmp(&right.candidate.stable_key));
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| TypeTuneError::new(format!("Could not format plan timestamp: {error}")))?;
    Ok(PlanFile {
        version: 1,
        project_path,
        created_at,
        options,
        entries,
    })
}

/// Creates a plan freshness fingerprint for a candidate and its local dependencies.
///
/// # Errors
///
/// Returns an error when the declaration is absent from the supplied source.
pub fn create_source_fingerprint(candidate: &Candidate, source: &str) -> Result<String> {
    let declarations = parse_type_declarations(source)?;
    let mut declarations_by_key = BTreeMap::new();
    for declaration in &declarations {
        declarations_by_key
            .entry(declaration_key(&candidate.source_path, declaration))
            .or_insert(declaration.declaration_text.as_str());
    }
    let keys = std::iter::once(candidate.declaration_key.as_str())
        .chain(
            candidate
                .local_dependency_declaration_keys
                .iter()
                .map(String::as_str),
        )
        .collect::<Vec<_>>();
    let texts = keys
        .into_iter()
        .map(|key| {
            declarations_by_key.get(key).copied().ok_or_else(|| {
                TypeTuneError::new(format!(
                    "Could not find declaration for key {key} in {}.",
                    candidate.source_path
                ))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(sha256(
        &std::iter::once(candidate.source_path.as_str())
            .chain(texts)
            .collect::<Vec<_>>()
            .join("\0"),
    ))
}

fn declaration_key(source_path: &str, declaration: &crate::parser::ParsedDeclaration) -> String {
    sha256(&declaration.stable_key(source_path))
}

fn default_ignore_status() -> String {
    "ignore".into()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{destination::PlacementStyle, scan::scan_declarations};

    use super::{
        PlanFile, PlanOptions, build_plan, merge_plan_markdown_state, read_plan_json,
        render_plan_markdown, validate_plan_candidate_metadata, validate_plan_freshness,
    };

    #[test]
    fn builds_sorted_ignored_plan_entries() {
        let source = "export type Shared = {};";
        let candidate = scan_declarations([("src/shared.ts".into(), source.into())])
            .expect("scans")
            .remove(0);
        let plan = build_plan(
            "tsconfig.json".into(),
            vec![candidate],
            &BTreeMap::from([("src/shared.ts".into(), source.into())]),
            PlanOptions {
                project_path: "tsconfig.json".into(),
                min_referrers: 2,
                referrer_count_exclude_globs: vec![],
                placement_style: PlacementStyle::Collocation,
                include_kinds: vec!["typeAlias".into()],
                ignore_entries: vec!["src/shared.ts#Shared".into()],
                default_status: "ignore".into(),
                include_existing_types: false,
                include_generated: false,
            },
        )
        .expect("builds");
        assert_eq!(plan.version, 1);
        assert_eq!(plan.entries[0].status, "ignore");
        assert_eq!(plan.entries[0].candidate.symbol_name, "Shared");
        assert!(render_plan_markdown(&plan).contains("- [ignore]"));
        let serialized = serde_json::to_string(&plan).expect("serializes");
        assert_eq!(read_plan_json(&serialized).expect("reads"), plan);
        let mut invalid_options = plan.options.clone();
        invalid_options.default_status = "invalid".into();
        assert!(
            build_plan(
                "tsconfig.json".into(),
                vec![],
                &BTreeMap::new(),
                invalid_options
            )
            .is_err()
        );
        let candidate = plan.entries[0].candidate.clone();
        let error = build_plan(
            "tsconfig.json".into(),
            vec![candidate.clone(), candidate],
            &BTreeMap::new(),
            plan.options.clone(),
        )
        .expect_err("rejects duplicates");
        assert!(error.to_string().contains("Duplicate candidate id"));
        let mut duplicate_plan = plan.clone();
        duplicate_plan.entries.push(plan.entries[0].clone());
        let serialized = serde_json::to_string(&duplicate_plan).expect("serializes duplicates");
        assert!(
            read_plan_json(&serialized)
                .expect_err("rejects duplicates")
                .to_string()
                .contains("Duplicate candidate id")
        );
        assert!(
            validate_plan_freshness(
                &plan,
                &BTreeMap::from([("src/shared.ts".into(), source.into())])
            )
            .is_ok()
        );
        assert!(
            validate_plan_candidate_metadata(&plan, &[plan.entries[0].candidate.clone()]).is_ok()
        );
    }

    #[test]
    fn merges_reviewed_markdown_statuses() {
        let source = "export type Shared = {};";
        let candidate = scan_declarations([("src/shared.ts".into(), source.into())])
            .expect("scans")
            .remove(0);
        let plan = build_plan(
            "tsconfig.json".into(),
            vec![candidate],
            &BTreeMap::from([("src/shared.ts".into(), source.into())]),
            PlanOptions {
                project_path: "tsconfig.json".into(),
                min_referrers: 2,
                referrer_count_exclude_globs: vec![],
                placement_style: PlacementStyle::Collocation,
                include_kinds: vec!["typeAlias".into()],
                ignore_entries: vec![],
                default_status: "ignore".into(),
                include_existing_types: false,
                include_generated: false,
            },
        )
        .expect("builds plan");
        let markdown = render_plan_markdown(&plan).replace("[ignore]", "[apply]");
        let reviewed = merge_plan_markdown_state(&plan, &markdown).expect("merges markdown");
        assert_eq!(reviewed.entries[0].status, "apply");
        assert_eq!(reviewed.entries[0].reviewed_destination, None);
        let proposed = &plan.entries[0].candidate.destination_path;
        let custom = markdown.replace(proposed, "src/domain/types/new.ts");
        let reviewed = merge_plan_markdown_state(&plan, &custom).expect("merges destination");
        assert_eq!(reviewed.entries[0].candidate, plan.entries[0].candidate);
        assert_eq!(
            reviewed.entries[0].source_fingerprint,
            plan.entries[0].source_fingerprint
        );
        assert_eq!(
            reviewed.entries[0].reviewed_destination.as_deref(),
            Some("src/domain/types/new.ts")
        );
        let json = serde_json::to_string(&reviewed).expect("serializes");
        assert!(!json.contains("reviewedDestination"));
        assert_eq!(
            read_plan_json(&json).expect("reads legacy format").entries[0].reviewed_destination,
            None
        );
        for replacement in [
            "../outside.ts",
            "/absolute.ts",
            "src/./bad.ts",
            "src/types.js",
            "src/a.d.ts",
        ] {
            assert!(
                merge_plan_markdown_state(&plan, &markdown.replace(proposed, replacement)).is_err(),
                "{replacement}"
            );
        }
        let destination_line = format!("  - to: `{proposed}`");
        for replacement in [
            String::new(),
            "  - to: invalid".into(),
            format!("{destination_line}\n{destination_line}"),
            "  - to: ``".into(),
        ] {
            assert!(
                merge_plan_markdown_state(
                    &plan,
                    &markdown.replace(&destination_line, &replacement)
                )
                .is_err(),
                "{replacement}"
            );
        }
        let duplicate = format!("{markdown}\n{markdown}");
        assert!(merge_plan_markdown_state(&plan, &duplicate).is_err());
        let mut duplicate_plan = plan.clone();
        duplicate_plan.entries.push(plan.entries[0].clone());
        let error = merge_plan_markdown_state(&duplicate_plan, &markdown)
            .expect_err("rejects duplicate plan IDs");
        assert!(error.to_string().contains("Duplicate candidate id"));
    }

    #[test]
    fn renders_ai_review_annotations_in_markdown() {
        let source = "export type Shared = {};";
        let candidate = scan_declarations([("src/shared.ts".into(), source.into())])
            .expect("scans")
            .remove(0);
        let mut plan = build_plan(
            "tsconfig.json".into(),
            vec![candidate],
            &BTreeMap::from([("src/shared.ts".into(), source.into())]),
            PlanOptions {
                project_path: "tsconfig.json".into(),
                min_referrers: 1,
                referrer_count_exclude_globs: vec![],
                placement_style: PlacementStyle::Collocation,
                include_kinds: vec!["typeAlias".into()],
                ignore_entries: vec![],
                default_status: "ignore".into(),
                include_existing_types: false,
                include_generated: false,
            },
        )
        .expect("builds plan");
        plan.entries[0].ai_review = Some(crate::ai::AiReviewDecision {
            candidate_id: plan.entries[0].candidate.id.clone(),
            status: "needs-review".into(),
            confidence: 0.0,
            reason: "Inspect manually.".into(),
            concerns: vec!["Provider response was incomplete.".into()],
            provider: "mock".into(),
            model: "mock-model".into(),
            reviewed_at: "2026-10-19T00:00:00Z".into(),
        });
        let markdown = render_plan_markdown(&plan);
        assert!(markdown.contains("AI: `needs-review`, confidence: 0"));
        assert!(markdown.contains("AI reason: Inspect manually."));
        assert!(markdown.contains("AI concerns: Provider response was incomplete."));
    }

    #[test]
    fn rejects_unknown_reviewed_markdown_candidates() {
        let plan = PlanFile {
            version: 1,
            project_path: "tsconfig.json".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            options: PlanOptions {
                project_path: "tsconfig.json".into(),
                min_referrers: 2,
                referrer_count_exclude_globs: vec![],
                placement_style: PlacementStyle::Collocation,
                include_kinds: vec![],
                ignore_entries: vec![],
                default_status: "ignore".into(),
                include_existing_types: false,
                include_generated: false,
            },
            entries: vec![],
        };
        assert!(merge_plan_markdown_state(&plan, "- [apply] unknown `Shared`\n").is_err());
    }

    #[test]
    fn rejects_stale_plan_entries() {
        let source = "export type Shared = {};";
        let candidate = scan_declarations([("src/shared.ts".into(), source.into())])
            .expect("scans")
            .remove(0);
        let plan = build_plan(
            "tsconfig.json".into(),
            vec![candidate],
            &BTreeMap::from([("src/shared.ts".into(), source.into())]),
            PlanOptions {
                project_path: "tsconfig.json".into(),
                min_referrers: 2,
                referrer_count_exclude_globs: vec![],
                placement_style: PlacementStyle::Collocation,
                include_kinds: vec!["typeAlias".into()],
                ignore_entries: vec![],
                default_status: "apply".into(),
                include_existing_types: false,
                include_generated: false,
            },
        )
        .expect("builds");
        assert!(
            validate_plan_freshness(
                &plan,
                &BTreeMap::from([(
                    "src/shared.ts".into(),
                    "export type Shared = string;".into()
                )])
            )
            .is_err()
        );
    }

    #[test]
    fn fingerprints_local_type_dependencies() {
        let source = "type Detail = string; export type Shared = Detail;";
        let candidate = scan_declarations([("src/shared.ts".into(), source.into())])
            .expect("scans")
            .remove(0);
        let plan = build_plan(
            "tsconfig.json".into(),
            vec![candidate],
            &BTreeMap::from([("src/shared.ts".into(), source.into())]),
            PlanOptions {
                project_path: "tsconfig.json".into(),
                min_referrers: 2,
                referrer_count_exclude_globs: vec![],
                placement_style: PlacementStyle::Collocation,
                include_kinds: vec!["typeAlias".into()],
                ignore_entries: vec![],
                default_status: "apply".into(),
                include_existing_types: false,
                include_generated: false,
            },
        )
        .expect("builds");
        assert!(
            validate_plan_freshness(
                &plan,
                &BTreeMap::from([(
                    "src/shared.ts".into(),
                    "type Detail = number; export type Shared = Detail;".into()
                )])
            )
            .is_err()
        );
    }
}
