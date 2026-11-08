use std::collections::BTreeMap;

use serde_json::json;

use crate::{destination::PlacementStyle, plan::PlanOptions, scan::scan_declarations};

use super::{
    AiReviewProviderOptions, build_http_request, build_request, mark_error, merge_decisions,
    parse_http_response, redact, review, review_with_cache,
};

#[test]
fn redacts_secret_values_without_removing_surrounding_code() {
    assert_eq!(
        redact(
            "const OPENAI_API_KEY = \"secret\";\nconst header = { authorization: 'Bearer secret' };\nconst value = process.env.INTERNAL_TOKEN;"
        ),
        "const OPENAI_API_KEY = \"[REDACTED]\";\nconst header = { authorization: '[REDACTED]' };\n[REDACTED: process.env reference]"
    );
}

fn plan() -> crate::plan::PlanFile {
    let source = "export type Shared = { token: string };";
    let candidate = scan_declarations([
        ("src/shared.ts".into(), source.into()),
        (
            "src/app.ts".into(),
            "import type { Shared } from './shared'; export type App = Shared;".into(),
        ),
    ])
    .expect("scans")
    .into_iter()
    .find(|candidate| candidate.symbol_name == "Shared")
    .expect("candidate");
    crate::plan::build_plan(
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
    .expect("builds plan")
}

#[test]
fn creates_redacted_and_byte_limited_snippet_context() {
    let plan = plan();
    let request = build_request(
        &plan,
        &BTreeMap::from([
            (
                "src/shared.ts".into(),
                "export type Shared = { token: string };".into(),
            ),
            (
                "src/app.ts".into(),
                "import type { Shared } from './shared'; export type App = Shared;".into(),
            ),
        ]),
        "snippets",
        None,
        Some(12),
    )
    .expect("builds request");
    let candidate = &request.candidates[0];
    assert_eq!(request.policy.context_truncated, Some(true));
    assert!(candidate.source_context_bytes.is_some());
    assert!(candidate.included_context_bytes.expect("byte count") <= 12);
    assert!(
        candidate
            .declaration_text
            .as_deref()
            .expect("declaration text")
            .starts_with("export type")
    );
}

#[test]
fn builds_ollama_http_requests() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let provider_request = build_http_request(
        &request,
        &AiReviewProviderOptions {
            provider: "ollama".into(),
            model: Some("qwen".into()),
            endpoint: None,
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
        "qwen",
    )
    .expect("builds provider request");

    assert_eq!(provider_request.endpoint, "http://127.0.0.1:11434/api/chat");
    assert_eq!(provider_request.headers["content-type"], "application/json");
    assert_eq!(provider_request.body["model"], "qwen");
    assert_eq!(provider_request.body["format"], "json");
    assert_eq!(provider_request.body["stream"], false);
}

#[test]
fn honors_custom_ollama_endpoints() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let provider_request = build_http_request(
        &request,
        &AiReviewProviderOptions {
            provider: "ollama".into(),
            model: Some("qwen".into()),
            endpoint: Some("http://ollama.internal/api/chat".into()),
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
        "qwen",
    )
    .expect("builds provider request");

    assert_eq!(provider_request.endpoint, "http://ollama.internal/api/chat");
}

#[test]
fn builds_openai_compatible_requests_without_auth_when_unconfigured() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let provider_request = build_http_request(
        &request,
        &AiReviewProviderOptions {
            provider: "openai-compatible".into(),
            model: Some("local-model".into()),
            endpoint: Some("http://localhost:8080/v1/chat/completions".into()),
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
        "local-model",
    )
    .expect("builds provider request");

    assert_eq!(
        provider_request.endpoint,
        "http://localhost:8080/v1/chat/completions"
    );
    assert_eq!(provider_request.headers.len(), 1);
    assert_eq!(provider_request.body["model"], "local-model");
    assert_eq!(
        provider_request.body["response_format"]["type"],
        "json_object"
    );
    assert_eq!(provider_request.body["messages"][0]["role"], "system");
}

#[test]
fn builds_openai_requests_with_the_configured_auth_header() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let provider_request = build_http_request(
        &request,
        &AiReviewProviderOptions {
            provider: "openai".into(),
            model: Some("gpt-test".into()),
            endpoint: Some("http://localhost:8080/openai".into()),
            api_key_env: Some("PATH".into()),
            response_file: None,
            timeout_ms: 1,
        },
        "gpt-test",
    )
    .expect("builds provider request");

    assert_eq!(provider_request.endpoint, "http://localhost:8080/openai");
    assert_eq!(provider_request.body["temperature"], 0);
    assert!(provider_request.headers["authorization"].starts_with("Bearer "));
}

#[test]
fn builds_anthropic_requests_with_required_headers() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let provider_request = build_http_request(
        &request,
        &AiReviewProviderOptions {
            provider: "anthropic".into(),
            model: Some("claude-test".into()),
            endpoint: Some("http://localhost:8080/anthropic".into()),
            api_key_env: Some("PATH".into()),
            response_file: None,
            timeout_ms: 1,
        },
        "claude-test",
    )
    .expect("builds provider request");

    assert_eq!(provider_request.body["model"], "claude-test");
    assert_eq!(provider_request.body["max_tokens"], 4096);
    assert_eq!(provider_request.headers["anthropic-version"], "2023-06-01");
    assert!(!provider_request.headers["x-api-key"].is_empty());
}

#[test]
fn parses_openai_completion_content() {
    let response = parse_http_response(
        "openai",
        json!({"choices": [{"message": {"content": "{\"decisions\":[]}"}}]}),
    )
    .expect("parses response");

    assert_eq!(response, json!({"decisions": []}));
}

#[test]
fn parses_anthropic_text_blocks() {
    let response = parse_http_response(
        "anthropic",
        json!({
            "content": [
                {"type": "thinking", "thinking": "ignored"},
                {"type": "text", "text": "{\"decisions\":"},
                {"type": "text", "text": "[]}"}
            ]
        }),
    )
    .expect("parses response");

    assert_eq!(response, json!({"decisions": []}));
}

#[test]
fn parses_ollama_message_content() {
    let response = parse_http_response(
        "ollama",
        json!({"message": {"content": "{\"decisions\":[]}"}}),
    )
    .expect("parses response");

    assert_eq!(response, json!({"decisions": []}));
}

#[test]
fn rejects_openai_responses_without_a_choice() {
    let error = parse_http_response("openai", json!({"choices": []}))
        .expect_err("rejects response without a completion");

    assert_eq!(
        error.to_string(),
        "Invalid AI review response. Expected at least one choice."
    );
}

#[test]
fn rejects_invalid_json_inside_openai_completions() {
    let error = parse_http_response(
        "openai-compatible",
        json!({"choices": [{"message": {"content": "not-json"}}]}),
    )
    .expect_err("rejects invalid nested JSON");

    assert!(
        error
            .to_string()
            .starts_with("Invalid AI review response JSON:")
    );
}

#[test]
fn rejects_anthropic_responses_without_text_content() {
    let error = parse_http_response("anthropic", json!({"content": [{"type": "tool_use"}]}))
        .expect_err("rejects response without text");

    assert_eq!(
        error.to_string(),
        "Invalid AI review response. Expected text content block."
    );
}

#[test]
fn rejects_ollama_responses_without_message_content() {
    let error = parse_http_response("ollama", json!({"message": {}}))
        .expect_err("rejects response without content");

    assert_eq!(
        error.to_string(),
        "Invalid AI review response. Expected message content string."
    );
}

#[test]
fn validates_mock_decisions_and_marks_missing_entries_for_review() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let response_path = std::env::temp_dir().join(format!(
        "type-tune-ai-review-{}-{}.json",
        std::process::id(),
        plan.entries[0].candidate.id
    ));
    std::fs::write(
        &response_path,
        format!(
            r#"{{"decisions":[{{"candidateId":"{}","status":"apply","confidence":0.8,"reason":"shared","concerns":[]}}]}}"#,
            plan.entries[0].candidate.id
        ),
    )
    .expect("writes response");
    let decisions = review(
        &request,
        &AiReviewProviderOptions {
            provider: "mock".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: Some(response_path.display().to_string()),
            timeout_ms: 1,
        },
    )
    .expect("validates response");
    let reviewed = merge_decisions(&plan, "mock", "mock-model", decisions);
    assert_eq!(reviewed.entries[0].status, "apply");
    assert_eq!(
        reviewed.entries[0]
            .ai_review
            .as_ref()
            .map(|review| review.status.as_str()),
        Some("apply")
    );
    let _ = std::fs::remove_file(response_path);
}

#[test]
fn reuses_cached_mock_decisions_without_reading_a_changed_response() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let root = std::env::temp_dir().join(format!(
        "type-tune-ai-cache-{}-{}",
        std::process::id(),
        plan.entries[0].candidate.id
    ));
    std::fs::create_dir_all(&root).expect("creates cache directory");
    let response_path = root.join("response.json");
    let cache_path = root.join("cache.json");
    std::fs::write(
        &response_path,
        format!(
            r#"{{"decisions":[{{"candidateId":"{}","status":"apply","confidence":0.8,"reason":"shared","concerns":[]}}]}}"#,
            plan.entries[0].candidate.id
        ),
    )
    .expect("writes response");
    let options = AiReviewProviderOptions {
        provider: "mock".into(),
        model: None,
        endpoint: None,
        api_key_env: None,
        response_file: Some(response_path.display().to_string()),
        timeout_ms: 1,
    };
    assert_eq!(
        review_with_cache(&plan, &request, &options, &cache_path).expect("caches decision")[0]
            .status,
        "apply"
    );
    std::fs::write(
        &response_path,
        r#"{"decisions":[{"candidateId":"unknown"}]}"#,
    )
    .expect("changes response");
    assert_eq!(
        review_with_cache(&plan, &request, &options, &cache_path).expect("reuses cached decision")
            [0]
        .status,
        "apply"
    );
    let mut cache: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cache_path).expect("reads cache"))
            .expect("parses cache");
    cache["entries"][0]["decision"]["candidateId"] = json!("different-candidate");
    std::fs::write(&cache_path, cache.to_string()).expect("writes mismatched cache");
    let error = review_with_cache(&plan, &request, &options, &cache_path)
        .expect_err("rejects mismatched cached decision");
    assert!(
        error
            .to_string()
            .contains("decision metadata does not match")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reuses_cached_decisions_with_empty_reasons() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let root = tempfile::tempdir().expect("creates fixture");
    let response_path = root.path().join("response.json");
    let cache_path = root.path().join("cache.json");
    let options = AiReviewProviderOptions {
        provider: "mock".into(),
        model: None,
        endpoint: None,
        api_key_env: None,
        response_file: Some(response_path.display().to_string()),
        timeout_ms: 1,
    };
    std::fs::write(
        &response_path,
        json!({"decisions": [{
            "candidateId": plan.entries[0].candidate.id,
            "status": "apply", "confidence": 0.8, "reason": "", "concerns": []
        }]})
        .to_string(),
    )
    .expect("writes response");
    review_with_cache(&plan, &request, &options, &cache_path).expect("caches decision");
    std::fs::write(&response_path, "invalid response").expect("changes response");
    let decisions =
        review_with_cache(&plan, &request, &options, &cache_path).expect("reuses decision");
    assert_eq!(decisions[0].reason, "");
}

#[test]
fn refreshes_cached_decision_when_candidate_context_changes() {
    let plan = plan();
    let mut request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let root = tempfile::tempdir().expect("creates fixture");
    let response_path = root.path().join("response.json");
    let cache_path = root.path().join("cache.json");
    let options = AiReviewProviderOptions {
        provider: "mock".into(),
        model: None,
        endpoint: None,
        api_key_env: None,
        response_file: Some(response_path.display().to_string()),
        timeout_ms: 1,
    };
    let response = |status: &str| {
        json!({"decisions": [{
            "candidateId": plan.entries[0].candidate.id, "status": status,
            "confidence": 0.8, "reason": "shared", "concerns": []
        }]})
        .to_string()
    };
    std::fs::write(&response_path, response("apply")).expect("writes response");
    review_with_cache(&plan, &request, &options, &cache_path).expect("caches decision");
    request.candidates[0].destination_path = "src/types/changed.ts".into();
    std::fs::write(&response_path, response("ignore")).expect("changes response");
    assert_eq!(
        review_with_cache(&plan, &request, &options, &cache_path).expect("reviews changed context")
            [0]
        .status,
        "ignore"
    );
}

#[test]
fn keeps_request_order_with_cached_and_fresh_decisions() {
    let mut plan = plan();
    let mut second = plan.entries[0].clone();
    second.candidate.id = "second-candidate".into();
    plan.entries.push(second);
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let root = tempfile::tempdir().expect("creates fixture");
    let response_path = root.path().join("response.json");
    let cache_path = root.path().join("cache.json");
    let options = AiReviewProviderOptions {
        provider: "mock".into(),
        model: None,
        endpoint: None,
        api_key_env: None,
        response_file: Some(response_path.display().to_string()),
        timeout_ms: 1,
    };
    let decision = |id: &str| {
        json!({"decisions": [{
            "candidateId": id, "status": "apply", "confidence": 0.8,
            "reason": "shared", "concerns": []
        }]})
        .to_string()
    };
    std::fs::write(
        &response_path,
        decision(&request.candidates[1].candidate_id),
    )
    .expect("writes cached response");
    let mut second_only = request.clone();
    second_only.candidates.remove(0);
    review_with_cache(&plan, &second_only, &options, &cache_path).expect("caches second");

    std::fs::write(
        &response_path,
        decision(&request.candidates[0].candidate_id),
    )
    .expect("writes fresh response");
    let decisions = review_with_cache(&plan, &request, &options, &cache_path).expect("reviews");
    assert_eq!(
        decisions
            .iter()
            .map(|decision| decision.candidate_id.as_str())
            .collect::<Vec<_>>(),
        request
            .candidates
            .iter()
            .map(|candidate| candidate.candidate_id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn cache_keys_distinguish_provider_endpoints() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let key = |endpoint| {
        super::cache::cache_key(
            "openai-compatible",
            "model",
            Some(endpoint),
            &request,
            &request.candidates[0],
            &plan.entries[0],
        )
        .expect("builds cache key")
    };
    assert_ne!(key("https://first.example"), key("https://second.example"));
}

#[test]
fn rejects_cached_decisions_with_an_invalid_status() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let root = std::env::temp_dir().join(format!(
        "type-tune-ai-invalid-cache-{}-{}",
        std::process::id(),
        plan.entries[0].candidate.id
    ));
    std::fs::create_dir_all(&root).expect("creates cache directory");
    let cache_path = root.join("cache.json");
    std::fs::write(
        &cache_path,
        r#"{"version":1,"entries":[{"key":"cached","decision":{"candidateId":"candidate","status":"unsafe","confidence":0.8,"reason":"reason","concerns":[],"provider":"mock","model":"mock-model","reviewedAt":"2026-01-01T00:00:00Z"}}]}"#,
    )
    .expect("writes cache");
    let error = review_with_cache(
        &plan,
        &request,
        &AiReviewProviderOptions {
            provider: "mock".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
        &cache_path,
    )
    .expect_err("rejects invalid cache");
    assert!(error.to_string().contains("Expected \"apply\""));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn marks_every_candidate_for_manual_review_after_provider_failure() {
    let reviewed = mark_error(&plan(), "mock", "mock-model", "response was malformed");
    assert_eq!(reviewed.entries[0].status, "ignore");
    assert_eq!(
        reviewed.entries[0]
            .ai_review
            .as_ref()
            .expect("review annotation")
            .status,
        "needs-review"
    );
    assert!(
        reviewed.entries[0]
            .ai_review
            .as_ref()
            .expect("review annotation")
            .concerns[0]
            .contains("response was malformed")
    );
}

#[test]
fn marks_missing_provider_decisions_as_needing_review() {
    let reviewed = merge_decisions(&plan(), "mock", "mock-model", vec![]);
    let decision = reviewed.entries[0]
        .ai_review
        .as_ref()
        .expect("review annotation");
    assert_eq!(reviewed.entries[0].status, "ignore");
    assert_eq!(decision.status, "needs-review");
    assert_eq!(
        decision.reason,
        "AI review response did not include a decision for this candidate."
    );
}

#[test]
fn rejects_mock_decisions_for_unknown_candidates() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let response_path = std::env::temp_dir().join(format!(
        "type-tune-ai-unknown-{}-{}.json",
        std::process::id(),
        plan.entries[0].candidate.id
    ));
    std::fs::write(
        &response_path,
        r#"{"decisions":[{"candidateId":"unknown","status":"apply","confidence":0.8,"reason":"shared","concerns":[]}]}"#,
    )
    .expect("writes response");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "mock".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: Some(response_path.display().to_string()),
            timeout_ms: 1,
        },
    )
    .expect_err("rejects unknown candidate");
    assert!(
        error
            .to_string()
            .contains("Unknown candidate id \"unknown\"")
    );
    let _ = std::fs::remove_file(response_path);
}

#[test]
fn rejects_duplicate_mock_decisions() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let response_path = std::env::temp_dir().join(format!(
        "type-tune-ai-duplicate-{}-{}.json",
        std::process::id(),
        plan.entries[0].candidate.id
    ));
    let decision = format!(
        r#"{{"candidateId":"{}","status":"apply","confidence":0.8,"reason":"shared","concerns":[]}}"#,
        plan.entries[0].candidate.id
    );
    std::fs::write(
        &response_path,
        format!(r#"{{"decisions":[{decision},{decision}]}}"#),
    )
    .expect("writes response");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "mock".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: Some(response_path.display().to_string()),
            timeout_ms: 1,
        },
    )
    .expect_err("rejects duplicate candidate");
    assert!(error.to_string().contains("Duplicate decision"));
    let _ = std::fs::remove_file(response_path);
}

#[test]
fn rejects_mock_decisions_with_out_of_range_confidence() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let response_path = std::env::temp_dir().join(format!(
        "type-tune-ai-confidence-{}-{}.json",
        std::process::id(),
        plan.entries[0].candidate.id
    ));
    std::fs::write(
        &response_path,
        format!(
            r#"{{"decisions":[{{"candidateId":"{}","status":"apply","confidence":1.1,"reason":"shared","concerns":[]}}]}}"#,
            plan.entries[0].candidate.id
        ),
    )
    .expect("writes response");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "mock".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: Some(response_path.display().to_string()),
            timeout_ms: 1,
        },
    )
    .expect_err("rejects confidence");
    assert!(error.to_string().contains("number from 0 to 1"));
    let _ = std::fs::remove_file(response_path);
}

#[test]
fn rejects_mock_decisions_with_an_invalid_status() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let response_path = std::env::temp_dir().join(format!(
        "type-tune-ai-status-{}-{}.json",
        std::process::id(),
        plan.entries[0].candidate.id
    ));
    std::fs::write(
        &response_path,
        format!(
            r#"{{"decisions":[{{"candidateId":"{}","status":"unsafe","confidence":0.8,"reason":"shared","concerns":[]}}]}}"#,
            plan.entries[0].candidate.id
        ),
    )
    .expect("writes response");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "mock".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: Some(response_path.display().to_string()),
            timeout_ms: 1,
        },
    )
    .expect_err("rejects status");
    assert!(error.to_string().contains("Expected \"apply\""));
    let _ = std::fs::remove_file(response_path);
}

#[test]
fn requires_a_model_for_network_providers_before_sending_a_request() {
    let request =
        build_request(&plan(), &BTreeMap::new(), "summary", None, None).expect("builds request");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "ollama".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
    )
    .expect_err("requires model");
    assert_eq!(
        error.to_string(),
        "--ai-model is required when using --ai-provider ollama."
    );
}

#[test]
fn rejects_unknown_providers_before_model_validation() {
    let plan = plan();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", None, None).expect("builds request");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "custom".into(),
            model: None,
            endpoint: None,
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
    )
    .expect_err("rejects unknown provider");

    assert!(
        error
            .to_string()
            .starts_with("AI review provider \"custom\" is not implemented yet.")
    );
}

#[test]
fn requires_an_endpoint_for_openai_compatible_review() {
    let request =
        build_request(&plan(), &BTreeMap::new(), "summary", None, None).expect("builds request");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "openai-compatible".into(),
            model: Some("local-model".into()),
            endpoint: None,
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
    )
    .expect_err("requires endpoint");
    assert_eq!(
        error.to_string(),
        "--ai-endpoint is required when using --ai-provider openai-compatible."
    );

    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "openai-compatible".into(),
            model: Some("local-model".into()),
            endpoint: Some(String::new()),
            api_key_env: None,
            response_file: None,
            timeout_ms: 1,
        },
    )
    .expect_err("requires non-empty endpoint");
    assert_eq!(
        error.to_string(),
        "--ai-endpoint is required when using --ai-provider openai-compatible."
    );
}

#[test]
fn requires_openai_api_key_before_sending_a_request() {
    let request =
        build_request(&plan(), &BTreeMap::new(), "summary", None, None).expect("builds request");
    let error = review(
        &request,
        &AiReviewProviderOptions {
            provider: "openai".into(),
            model: Some("test-model".into()),
            endpoint: None,
            api_key_env: Some("TYPE_TUNE_MISSING_OPENAI_KEY".into()),
            response_file: None,
            timeout_ms: 1,
        },
    )
    .expect_err("requires key");
    assert_eq!(
        error.to_string(),
        "Environment variable TYPE_TUNE_MISSING_OPENAI_KEY is required for AI review."
    );
}

#[test]
fn limits_requests_to_the_first_deterministic_candidates() {
    let source = "export type Alpha = {}; export type Beta = {};";
    let candidates = scan_declarations([
        ("src/shared.ts".into(), source.into()),
        (
            "src/app.ts".into(),
            "import type { Alpha, Beta } from './shared'; export type App = Alpha & Beta;".into(),
        ),
    ])
    .expect("scans");
    let plan = crate::plan::build_plan(
        "tsconfig.json".into(),
        candidates,
        &BTreeMap::from([
            ("src/shared.ts".into(), source.into()),
            (
                "src/app.ts".into(),
                "import type { Alpha, Beta } from './shared'; export type App = Alpha & Beta;"
                    .into(),
            ),
        ]),
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
    let first_symbol = plan.entries[0].candidate.symbol_name.clone();
    let request =
        build_request(&plan, &BTreeMap::new(), "summary", Some(1), None).expect("builds request");
    assert_eq!(request.candidates.len(), 1);
    assert_eq!(request.candidates[0].symbol_name, first_symbol);
}
