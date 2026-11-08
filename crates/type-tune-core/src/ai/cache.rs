//! AI decision cache persistence and keys.

use std::collections::BTreeSet;

use super::{
    AiReviewCacheFile, AiReviewCandidateContext, AiReviewDecision, AiReviewRequest,
    validate_decision_confidence, validate_decision_status,
};
use crate::{Result, TypeTuneError, hash::sha256, plan::PlanEntry};

pub(super) fn read_cache(path: &std::path::Path) -> Result<AiReviewCacheFile> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AiReviewCacheFile {
                version: 1,
                entries: vec![],
            });
        }
        Err(error) => {
            return Err(TypeTuneError::new(format!(
                "Could not read {}: {error}",
                path.display()
            )));
        }
    };
    let cache: AiReviewCacheFile = serde_json::from_str(&content)
        .map_err(|error| TypeTuneError::new(format!("Could not parse AI review cache: {error}")))?;
    if cache.version != 1 {
        return Err(TypeTuneError::new(
            "Invalid aiReviewCache.version. Expected 1.",
        ));
    }
    let mut keys = BTreeSet::new();
    for (index, entry) in cache.entries.iter().enumerate() {
        if entry.key.is_empty() {
            return Err(TypeTuneError::new(format!(
                "Invalid aiReviewCache.entries[{index}].key. Expected a string."
            )));
        }
        if !keys.insert(entry.key.as_str()) {
            return Err(TypeTuneError::new(format!(
                "Duplicate AI review cache key at aiReviewCache.entries[{index}].key."
            )));
        }
        validate_cached_decision(&entry.decision, index)?;
    }
    Ok(cache)
}

#[cfg(test)]
mod tests {
    use super::read_cache;

    #[test]
    fn distinguishes_missing_cache_from_invalid_parent() {
        let root = tempfile::tempdir().expect("creates fixture");
        assert!(
            read_cache(&root.path().join("missing.json"))
                .expect("missing cache")
                .entries
                .is_empty()
        );
        #[cfg(unix)]
        {
            let parent = root.path().join("file");
            std::fs::write(&parent, "not a directory").expect("writes fixture");
            assert!(read_cache(&parent.join("cache.json")).is_err());
        }
    }

    #[test]
    fn rejects_duplicate_cache_keys() {
        let root = tempfile::tempdir().expect("creates fixture");
        let path = root.path().join("cache.json");
        let entry = serde_json::json!({
            "key": "duplicate",
            "decision": {
                "candidateId": "candidate", "status": "apply", "confidence": 0.8,
                "reason": "reviewed", "concerns": [], "provider": "mock",
                "model": "mock-model", "reviewedAt": "2026-11-06T00:00:00Z"
            }
        });
        std::fs::write(
            &path,
            serde_json::json!({"version": 1, "entries": [entry.clone(), entry]}).to_string(),
        )
        .expect("writes cache");
        assert!(
            read_cache(&path)
                .expect_err("rejects duplicates")
                .to_string()
                .contains("Duplicate AI review cache key")
        );
    }
}

fn validate_cached_decision(decision: &AiReviewDecision, index: usize) -> Result<()> {
    let path = format!("aiReviewCache.entries[{index}].decision");
    if decision.candidate_id.is_empty()
        || decision.provider.is_empty()
        || decision.model.is_empty()
        || decision.reviewed_at.is_empty()
    {
        return Err(TypeTuneError::new(format!(
            "Invalid {path}. Expected non-empty decision fields."
        )));
    }
    validate_decision_status(&decision.status, &path)?;
    validate_decision_confidence(decision.confidence, &path)
}

pub(super) fn write_cache(path: &std::path::Path, cache: &AiReviewCacheFile) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            TypeTuneError::new(format!("Could not create {}: {error}", parent.display()))
        })?;
    }
    let content = serde_json::to_string_pretty(cache).map_err(|error| {
        TypeTuneError::new(format!("Could not serialize AI review cache: {error}"))
    })?;
    std::fs::write(path, format!("{content}\n"))
        .map_err(|error| TypeTuneError::new(format!("Could not write {}: {error}", path.display())))
}

pub(super) fn cache_key(
    provider: &str,
    model: &str,
    endpoint: Option<&str>,
    request: &AiReviewRequest,
    candidate: &AiReviewCandidateContext,
    entry: &PlanEntry,
) -> Result<String> {
    let key_input = serde_json::to_string(&(
        provider,
        model,
        endpoint,
        &request.project_path,
        &request.placement_style,
        &request.policy,
        candidate,
        &entry.candidate.declaration_key,
        &entry.source_fingerprint,
    ))
    .map_err(|error| TypeTuneError::new(format!("Could not serialize AI cache key: {error}")))?;
    Ok(sha256(&key_input))
}
