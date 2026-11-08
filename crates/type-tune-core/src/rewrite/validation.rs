//! Validation of rewrite candidates and declaration identities.

use crate::{
    Result, TypeTuneError,
    hash::sha256,
    model::{Candidate, ImportStrategy},
    path::normalize_project_path,
};
use std::collections::{BTreeMap, HashSet};

pub(super) struct CandidateGroups<'a> {
    pub(super) by_source: BTreeMap<String, Vec<&'a Candidate>>,
    pub(super) destinations: BTreeMap<String, String>,
}

/// Validate candidates and group them by source while ensuring a single destination per source.
pub(super) fn group_candidates(candidates: &[Candidate]) -> Result<CandidateGroups<'_>> {
    let mut candidates_by_source = BTreeMap::<String, Vec<&Candidate>>::new();
    for candidate in candidates {
        validate_candidate_paths(candidate)?;
        candidates_by_source
            .entry(candidate.source_path.clone())
            .or_default()
            .push(candidate);
    }
    assert_no_destination_symbol_conflicts(candidates)?;
    let moved_destination_paths = candidates_by_source
        .iter()
        .map(|(source_path, source_candidates)| {
            let destination_path = &source_candidates[0].destination_path;
            if source_candidates
                .iter()
                .any(|candidate| candidate.destination_path != *destination_path)
            {
                return Err(TypeTuneError::new(format!(
                    "Native rewrite requires candidates from {source_path} to share one destination."
                )));
            }
            Ok((source_path.clone(), destination_path.clone()))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok(CandidateGroups {
        by_source: candidates_by_source,
        destinations: moved_destination_paths,
    })
}

pub(super) fn validate_candidate_paths(candidate: &Candidate) -> Result<()> {
    validate_canonical_path(&candidate.source_path, "sourcePath")?;
    validate_canonical_path(&candidate.destination_path, "destinationPath")?;
    for referrer in &candidate.referrer_strategies {
        validate_canonical_path(&referrer.referrer_path, "referrerPath")?;
        if let ImportStrategy::Barrel {
            barrel_file_path, ..
        } = &referrer.strategy
        {
            validate_canonical_path(barrel_file_path, "barrelFilePath")?;
        }
    }
    Ok(())
}

fn validate_canonical_path(path: &str, field: &str) -> Result<()> {
    let normalized = normalize_project_path(path)?;
    if normalized != path {
        return Err(TypeTuneError::new(format!(
            "Expected canonical project-relative {field}: {path}."
        )));
    }
    Ok(())
}

pub(super) fn declaration_key(
    source_path: &str,
    declaration: &crate::parser::ParsedDeclaration,
) -> Option<String> {
    (!declaration.symbol_name.is_empty()).then(|| sha256(&declaration.stable_key(source_path)))
}

pub(super) fn moved_declarations<'a>(
    source_path: &str,
    candidates: &[&Candidate],
    declarations: &'a [crate::parser::ParsedDeclaration],
) -> Result<Vec<&'a crate::parser::ParsedDeclaration>> {
    let moved_keys = candidates
        .iter()
        .flat_map(|candidate| {
            std::iter::once(candidate.declaration_key.as_str()).chain(
                candidate
                    .local_dependency_declaration_keys
                    .iter()
                    .map(String::as_str),
            )
        })
        .collect::<HashSet<_>>();
    let moved = declarations
        .iter()
        .filter(|declaration| {
            declaration_key(source_path, declaration)
                .is_some_and(|key| moved_keys.contains(key.as_str()))
        })
        .collect::<Vec<_>>();
    if moved.len() != moved_keys.len() {
        return Err(TypeTuneError::new(format!(
            "Could not find all moved declarations for {source_path}."
        )));
    }
    Ok(moved)
}

pub(super) fn assert_no_destination_symbol_conflicts(candidates: &[Candidate]) -> Result<()> {
    let mut planned_symbols = BTreeMap::<(&str, &str), (&str, &str)>::new();
    for candidate in candidates {
        let symbol_key = (
            candidate.destination_path.as_str(),
            candidate.symbol_name.as_str(),
        );
        if let Some((existing_source, existing_declaration_key)) = planned_symbols.get(&symbol_key)
        {
            if candidate.declaration_key != *existing_declaration_key {
                return Err(TypeTuneError::new(format!(
                    "Conflicting declarations named {} would be written to {}. Existing planned declaration: {}#{}. New planned declaration: {}#{}.",
                    candidate.symbol_name,
                    candidate.destination_path,
                    existing_source,
                    candidate.symbol_name,
                    candidate.source_path,
                    candidate.symbol_name,
                )));
            }
            continue;
        }
        planned_symbols.insert(
            symbol_key,
            (
                candidate.source_path.as_str(),
                candidate.declaration_key.as_str(),
            ),
        );
    }
    Ok(())
}
