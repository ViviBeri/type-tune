//! HTTP provider request and response adapters.

use super::{AiReviewProviderOptions, AiReviewRequest, SYSTEM_PROMPT};
use crate::{Result, TypeTuneError};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProviderHttpRequest {
    pub(super) endpoint: String,
    pub(super) body: Value,
    pub(super) headers: BTreeMap<&'static str, String>,
}

pub(super) fn call_http_provider(
    request: &AiReviewRequest,
    options: &AiReviewProviderOptions,
    model: &str,
) -> Result<Value> {
    let provider_request = build_http_request(request, options, model)?;
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(options.timeout_ms))
        .build();
    let response = provider_request
        .headers
        .into_iter()
        .fold(
            agent.post(&provider_request.endpoint),
            |request, (key, value)| request.set(key, &value),
        )
        .send_json(provider_request.body);
    let text = match response {
        Ok(response) => response.into_string().map_err(|error| {
            TypeTuneError::new(format!("Could not read AI review response: {error}"))
        })?,
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            return Err(TypeTuneError::new(format!(
                "AI review request failed with HTTP {status}: {body}"
            )));
        }
        Err(error) => {
            return Err(TypeTuneError::new(format!(
                "AI review request failed: {error}"
            )));
        }
    };
    let response: Value = serde_json::from_str(&text)
        .map_err(|error| TypeTuneError::new(format!("Invalid AI review response JSON: {error}")))?;
    parse_http_response(&options.provider, response)
}

pub(super) fn build_http_request(
    request: &AiReviewRequest,
    options: &AiReviewProviderOptions,
    model: &str,
) -> Result<ProviderHttpRequest> {
    let request_json = serde_json::to_string(request).map_err(|error| {
        TypeTuneError::new(format!("Could not serialize AI review request: {error}"))
    })?;
    let (endpoint, body, headers) = match options.provider.as_str() {
        "openai" | "openai-compatible" => {
            let endpoint = options
                .endpoint
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".into());
            let mut headers = BTreeMap::from([("content-type", "application/json".to_string())]);
            if options.provider == "openai" || options.api_key_env.is_some() {
                let name = options.api_key_env.as_deref().unwrap_or("OPENAI_API_KEY");
                let key = std::env::var(name).map_err(|_| {
                    TypeTuneError::new(format!(
                        "Environment variable {name} is required for AI review."
                    ))
                })?;
                headers.insert("authorization", format!("Bearer {key}"));
            }
            (
                endpoint,
                json!({"model": model, "temperature": 0, "response_format": {"type": "json_object"}, "messages": [{"role": "system", "content": SYSTEM_PROMPT}, {"role": "user", "content": request_json}]}),
                headers,
            )
        }
        "anthropic" => {
            let endpoint = options
                .endpoint
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".into());
            let name = options
                .api_key_env
                .as_deref()
                .unwrap_or("ANTHROPIC_API_KEY");
            let key = std::env::var(name).map_err(|_| {
                TypeTuneError::new(format!(
                    "Environment variable {name} is required for AI review."
                ))
            })?;
            (
                endpoint,
                json!({"model": model, "max_tokens": 4096, "temperature": 0, "system": SYSTEM_PROMPT, "messages": [{"role": "user", "content": request_json}]}),
                BTreeMap::from([
                    ("content-type", "application/json".into()),
                    ("anthropic-version", "2023-06-01".into()),
                    ("x-api-key", key),
                ]),
            )
        }
        "ollama" => (
            options
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:11434/api/chat".into()),
            json!({"model": model, "stream": false, "format": "json", "options": {"temperature": 0}, "messages": [{"role": "system", "content": SYSTEM_PROMPT}, {"role": "user", "content": request_json}]}),
            BTreeMap::from([("content-type", "application/json".into())]),
        ),
        _ => unreachable!(),
    };
    Ok(ProviderHttpRequest {
        endpoint,
        body,
        headers,
    })
}

pub(super) fn parse_http_response(provider: &str, response: Value) -> Result<Value> {
    match provider {
        "openai" | "openai-compatible" => nested_string_json(
            &response,
            &["choices", "0", "message", "content"],
            "Expected at least one choice.",
        ),
        "anthropic" => {
            let blocks = response
                .get("content")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    TypeTuneError::new("Invalid AI review response. Expected content array.")
                })?;
            let content = blocks
                .iter()
                .filter_map(|block| {
                    (block.get("type").and_then(Value::as_str) == Some("text"))
                        .then(|| block.get("text").and_then(Value::as_str))
                        .flatten()
                })
                .collect::<String>();
            if content.is_empty() {
                return Err(TypeTuneError::new(
                    "Invalid AI review response. Expected text content block.",
                ));
            }
            serde_json::from_str(&content).map_err(|error| {
                TypeTuneError::new(format!("Invalid AI review response JSON: {error}"))
            })
        }
        "ollama" => nested_string_json(
            &response,
            &["message", "content"],
            "Expected message content string.",
        ),
        _ => unreachable!(),
    }
}

fn nested_string_json(value: &Value, path: &[&str], expected: &str) -> Result<Value> {
    let mut current = value;
    for segment in path {
        current = if let Ok(index) = segment.parse::<usize>() {
            current.as_array().and_then(|items| items.get(index))
        } else {
            current.get(*segment)
        }
        .ok_or_else(|| TypeTuneError::new(format!("Invalid AI review response. {expected}")))?;
    }
    let text = current
        .as_str()
        .ok_or_else(|| TypeTuneError::new(format!("Invalid AI review response. {expected}")))?;
    serde_json::from_str(text)
        .map_err(|error| TypeTuneError::new(format!("Invalid AI review response JSON: {error}")))
}
