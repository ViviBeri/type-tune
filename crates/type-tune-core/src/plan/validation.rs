//! Validate serialized plans and their candidates before application.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use crate::{
    Result, TypeTuneError,
    model::Candidate,
    path::{validate_project_relative_typescript_path, visit_candidate_paths},
};

use super::{PlanFile, create_source_fingerprint};

/// Parses and validates a serialized plan file.
///
/// # Errors
///
/// Returns an error for invalid JSON, unsupported versions, invalid statuses, or duplicate IDs.
pub fn read_plan_json(content: &str) -> Result<PlanFile> {
    let plan: PlanFile = serde_json::from_str(content)
        .map_err(|error| TypeTuneError::new(format!("Could not parse plan JSON: {error}")))?;
    if plan.version != 1 {
        return Err(TypeTuneError::new(format!(
            "Unsupported plan version {}. Expected 1.",
            plan.version
        )));
    }
    if plan
        .entries
        .iter()
        .any(|entry| !matches!(entry.status.as_str(), "apply" | "ignore"))
    {
        return Err(TypeTuneError::new(
            "Invalid plan entry status. Expected \"apply\" or \"ignore\".",
        ));
    }
    let mut ids = BTreeSet::new();
    for (index, entry) in plan.entries.iter().enumerate() {
        if !ids.insert(entry.candidate.id.as_str()) {
            return Err(TypeTuneError::new(format!(
                "Duplicate candidate id \"{}\" in plan entries.",
                entry.candidate.id
            )));
        }
        validate_plan_candidate_paths(
            &entry.candidate,
            &format!("plan.entries[{index}].candidate"),
        )?;
    }
    Ok(plan)
}

fn validate_plan_candidate_paths(candidate: &Candidate, path: &str) -> Result<()> {
    visit_candidate_paths(candidate, path, |file_path, field_path| {
        validate_project_relative_typescript_path(file_path, field_path).map(|_| ())
    })
}

/// Validates that candidate paths stay within the project root before apply.
///
/// # Errors
///
/// Returns an error when a candidate path escapes the project, including through symbolic links.
pub fn validate_plan_candidate_path_boundaries(
    project_root: &Path,
    candidates: &[Candidate],
) -> Result<()> {
    for (index, candidate) in candidates.iter().enumerate() {
        crate::path::validate_candidate_path_boundaries(
            project_root,
            candidate,
            &format!("candidates[{index}]"),
        )?;
    }
    Ok(())
}

/// Validates that all native plan source fingerprints match current source text.
///
/// # Errors
///
/// Returns an error for missing sources or stale declarations.
pub fn validate_plan_freshness(plan: &PlanFile, sources: &BTreeMap<String, String>) -> Result<()> {
    for entry in &plan.entries {
        let source = sources.get(&entry.candidate.source_path).ok_or_else(|| {
            TypeTuneError::new(format!(
                "Could not find source for {}.",
                entry.candidate.source_path
            ))
        })?;
        let fingerprint = create_source_fingerprint(&entry.candidate, source)?;
        if fingerprint != entry.source_fingerprint {
            return Err(TypeTuneError::new(format!(
                "Stale plan entry for candidate {} ({}). Regenerate the plan before applying.",
                entry.candidate.id, entry.candidate.symbol_name
            )));
        }
    }
    Ok(())
}

/// Validates that persisted candidate metadata matches a current native scan.
///
/// # Errors
///
/// Returns an error when any plan candidate is absent or differs from current scan output.
pub fn validate_plan_candidate_metadata(plan: &PlanFile, candidates: &[Candidate]) -> Result<()> {
    let candidates_by_id = candidates
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    for entry in &plan.entries {
        let current = candidates_by_id.get(entry.candidate.id.as_str()).ok_or_else(|| {
            TypeTuneError::new(format!(
                "Stale plan entry for candidate {} ({}). Current scan did not return this candidate.",
                entry.candidate.id, entry.candidate.symbol_name
            ))
        })?;
        if *current != &entry.candidate {
            return Err(TypeTuneError::new(format!(
                "Invalid candidate metadata for {} ({}). Regenerate the plan before applying.",
                entry.candidate.id, entry.candidate.symbol_name
            )));
        }
    }
    Ok(())
}
