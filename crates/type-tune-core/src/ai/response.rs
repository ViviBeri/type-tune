//! Validate provider responses against the submitted review request.

use std::collections::BTreeSet;

use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{Result, TypeTuneError};

use super::{
    AiReviewDecision, AiReviewRequest, validate_decision_confidence, validate_decision_status,
};

pub(super) fn validate_response(
    value: Value,
    request: &AiReviewRequest,
    provider: &str,
    model: &str,
) -> Result<Vec<AiReviewDecision>> {
    let decisions = value
        .get("decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TypeTuneError::new("Invalid aiReviewResponse.decisions. Expected an array.")
        })?;
    let expected = request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.as_str())
        .collect::<BTreeSet<_>>();
    let reviewed_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| {
            TypeTuneError::new(format!("Could not format AI review timestamp: {error}"))
        })?;
    let mut seen = BTreeSet::new();
    decisions
        .iter()
        .enumerate()
        .map(|(index, value)| {
            validate_decision(
                value,
                index,
                &expected,
                &mut seen,
                provider,
                model,
                &reviewed_at,
            )
        })
        .collect()
}

fn validate_decision(
    value: &Value,
    index: usize,
    expected: &BTreeSet<&str>,
    seen: &mut BTreeSet<String>,
    provider: &str,
    model: &str,
    reviewed_at: &str,
) -> Result<AiReviewDecision> {
    let path = format!("aiReviewResponse.decisions[{index}]");
    let object = value
        .as_object()
        .ok_or_else(|| TypeTuneError::new(format!("Invalid {path}. Expected an object.")))?;
    let candidate_id = string_field(object, "candidateId", &path)?;
    if !expected.contains(candidate_id.as_str()) {
        return Err(TypeTuneError::new(format!(
            "Invalid AI review response. Unknown candidate id \"{candidate_id}\"."
        )));
    }
    if !seen.insert(candidate_id.clone()) {
        return Err(TypeTuneError::new(format!(
            "Invalid AI review response. Duplicate decision for candidate id \"{candidate_id}\"."
        )));
    }
    let status = string_field(object, "status", &path)?;
    validate_decision_status(&status, &path)?;
    let confidence = object
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            TypeTuneError::new(format!("Invalid {path}.confidence. Expected a number."))
        })?;
    validate_decision_confidence(confidence, &path)?;
    let concerns = object
        .get("concerns")
        .and_then(Value::as_array)
        .ok_or_else(|| TypeTuneError::new(format!("Invalid {path}.concerns. Expected an array.")))?
        .iter()
        .enumerate()
        .map(|(item, concern)| {
            concern.as_str().map(str::to_owned).ok_or_else(|| {
                TypeTuneError::new(format!(
                    "Invalid {path}.concerns[{item}]. Expected a string."
                ))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(AiReviewDecision {
        candidate_id,
        status,
        confidence,
        reason: string_field(object, "reason", &path)?,
        concerns,
        provider: provider.into(),
        model: model.into(),
        reviewed_at: reviewed_at.into(),
    })
}

fn string_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
    path: &str,
) -> Result<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| TypeTuneError::new(format!("Invalid {path}.{field}. Expected a string.")))
}
