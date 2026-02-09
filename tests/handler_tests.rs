//! Integration tests for list_caches and inspect_cache handlers.
//!
//! Tests exercise the handler functions directly with a test ServerConfig,
//! and verify the full dispatch flow for tool calls.

use std::fs;
use std::path::Path;
use std::time::Duration;

use context_core::cache::{CacheBuildConfig, CacheBuilder};
use context_core::document::{Document, DocumentId, Metadata};
use mcp_context_server::config::ServerConfig;
use mcp_context_server::handlers;
use mcp_context_server::protocol::{InspectCacheParams, JsonRpcRequest, RpcId};

fn test_config(cache_root: &Path) -> ServerConfig {
    ServerConfig {
        cache_root: cache_root.to_path_buf(),
        tool_timeout: Duration::from_secs(30),
    }
}

fn build_test_cache(cache_dir: &Path) {
    let root = Path::new("/test");
    let docs = vec![
        Document::ingest(
            DocumentId::from_path(root, &root.join("docs/alpha.md")).unwrap(),
            "docs/alpha.md".to_string(),
            b"Alpha document content for testing".to_vec(),
            Metadata::default(),
        )
        .unwrap(),
        Document::ingest(
            DocumentId::from_path(root, &root.join("docs/beta.md")).unwrap(),
            "docs/beta.md".to_string(),
            b"Beta document content for testing".to_vec(),
            Metadata::default(),
        )
        .unwrap(),
    ];

    let builder = CacheBuilder::new(CacheBuildConfig::v0());
    builder.build(docs, cache_dir).unwrap();
}

// ---------------------------------------------------------------------------
// list_caches tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_caches_empty_root() {
    let tmp = tempfile::tempdir().unwrap();
    let config = test_config(tmp.path());

    let result = handlers::list_caches::handle(&config).await;
    assert!(!result.is_error, "list_caches should succeed on empty root");

    let text = &result.content[0].text;
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    let caches = value["caches"].as_array().unwrap();
    assert!(caches.is_empty(), "Empty root should produce empty caches array");
}

#[tokio::test]
async fn list_caches_finds_caches() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // Create two cache directories, one with manifest, one without
    let cache_a = root.join("cache-a");
    build_test_cache(&cache_a);

    let cache_b = root.join("cache-b");
    fs::create_dir_all(&cache_b).unwrap();
    // cache-b has no manifest

    // Create a regular file (should be ignored)
    fs::write(root.join("notes.txt"), "not a cache").unwrap();

    let config = test_config(root);
    let result = handlers::list_caches::handle(&config).await;
    assert!(!result.is_error);

    let text = &result.content[0].text;
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    let caches = value["caches"].as_array().unwrap();

    assert_eq!(caches.len(), 2, "Should find 2 directories (ignoring file)");

    // Verify sorted order
    assert_eq!(caches[0]["path"].as_str().unwrap(), "cache-a");
    assert_eq!(caches[1]["path"].as_str().unwrap(), "cache-b");

    // Verify has_manifest
    assert_eq!(caches[0]["has_manifest"].as_bool().unwrap(), true);
    assert_eq!(caches[1]["has_manifest"].as_bool().unwrap(), false);
}

#[tokio::test]
async fn list_caches_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    build_test_cache(&root.join("cache-x"));
    build_test_cache(&root.join("cache-y"));
    fs::create_dir_all(root.join("cache-z")).unwrap();

    let config = test_config(root);

    let result_a = handlers::list_caches::handle(&config).await;
    let result_b = handlers::list_caches::handle(&config).await;

    assert_eq!(
        result_a.content[0].text, result_b.content[0].text,
        "list_caches must produce byte-identical output across runs"
    );
}

#[tokio::test]
async fn list_caches_nonexistent_root() {
    let tmp = tempfile::tempdir().unwrap();
    let config = test_config(&tmp.path().join("nonexistent"));

    let result = handlers::list_caches::handle(&config).await;
    assert!(result.is_error, "Should error on nonexistent root");
}

// ---------------------------------------------------------------------------
// inspect_cache tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn inspect_cache_valid() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let cache_name = "my-cache";
    build_test_cache(&root.join(cache_name));

    let config = test_config(root);
    let params = InspectCacheParams {
        cache: cache_name.to_string(),
    };

    let result = handlers::inspect_cache::handle(params, &config).await;
    assert!(!result.is_error, "inspect_cache should succeed on valid cache");

    let text = &result.content[0].text;
    let value: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(value["cache_version"].as_str().unwrap().starts_with("sha256:"));
    assert_eq!(value["document_count"].as_u64().unwrap(), 2);
    assert!(value["total_bytes"].as_u64().unwrap() > 0);
    assert_eq!(value["valid"].as_bool().unwrap(), true);
}

#[tokio::test]
async fn inspect_cache_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let config = test_config(tmp.path());

    let params = InspectCacheParams {
        cache: "nonexistent".to_string(),
    };

    let result = handlers::inspect_cache::handle(params, &config).await;
    assert!(result.is_error, "Should error on missing cache");

    let text = &result.content[0].text;
    let err: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(err["error"]["code"].as_str().unwrap(), "cache_missing");
}

#[tokio::test]
async fn inspect_cache_invalid_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let cache_dir = root.join("bad-cache");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("manifest.json"), "not valid json").unwrap();

    let config = test_config(root);
    let params = InspectCacheParams {
        cache: "bad-cache".to_string(),
    };

    let result = handlers::inspect_cache::handle(params, &config).await;
    assert!(!result.is_error, "Invalid manifest returns valid=false, not a tool error");

    let text = &result.content[0].text;
    let value: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(value["cache_version"].as_str().unwrap(), "");
    assert_eq!(value["document_count"].as_u64().unwrap(), 0);
    assert_eq!(value["total_bytes"].as_u64().unwrap(), 0);
    assert_eq!(value["valid"].as_bool().unwrap(), false);
}

#[tokio::test]
async fn inspect_cache_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_test_cache(&root.join("det-cache"));

    let config = test_config(root);

    let params_a = InspectCacheParams {
        cache: "det-cache".to_string(),
    };
    let params_b = InspectCacheParams {
        cache: "det-cache".to_string(),
    };

    let result_a = handlers::inspect_cache::handle(params_a, &config).await;
    let result_b = handlers::inspect_cache::handle(params_b, &config).await;

    assert_eq!(
        result_a.content[0].text, result_b.content[0].text,
        "inspect_cache must produce byte-identical output across runs"
    );
}

#[tokio::test]
async fn inspect_cache_path_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let config = test_config(tmp.path());

    let traversal_attempts = vec![
        "../etc/passwd",
        "../../secret",
        "/absolute/path",
        "\\windows\\path",
    ];

    for attempt in traversal_attempts {
        let params = InspectCacheParams {
            cache: attempt.to_string(),
        };

        let result = handlers::inspect_cache::handle(params, &config).await;
        assert!(
            result.is_error,
            "Path traversal attempt '{}' should be rejected",
            attempt
        );
    }
}

// ---------------------------------------------------------------------------
// Dispatch integration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_tools_list_advertises_all_tools() {
    let tmp = tempfile::tempdir().unwrap();
    let config = test_config(tmp.path());

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(1)),
        method: "tools/list".into(),
        params: None,
    };

    let response = handlers::dispatch(&req, &config).await.unwrap();
    let result = response.result.unwrap();
    let tools = result["tools"].as_array().unwrap();

    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();

    assert!(tool_names.contains(&"context.resolve"), "Should advertise context.resolve");
    assert!(tool_names.contains(&"context.list_caches"), "Should advertise context.list_caches");
    assert!(tool_names.contains(&"context.inspect_cache"), "Should advertise context.inspect_cache");
    assert_eq!(tools.len(), 3, "Should advertise exactly 3 tools");
}

#[tokio::test]
async fn dispatch_list_caches_via_tools_call() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_test_cache(&root.join("test-cache"));

    let config = test_config(root);

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(2)),
        method: "tools/call".into(),
        params: Some(serde_json::json!({
            "name": "context.list_caches",
            "arguments": {}
        })),
    };

    let response = handlers::dispatch(&req, &config).await.unwrap();
    let result = response.result.unwrap();

    // Tool result contains content array
    let text = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let caches = parsed["caches"].as_array().unwrap();

    assert_eq!(caches.len(), 1);
    assert_eq!(caches[0]["path"].as_str().unwrap(), "test-cache");
    assert_eq!(caches[0]["has_manifest"].as_bool().unwrap(), true);
}

#[tokio::test]
async fn dispatch_inspect_cache_via_tools_call() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_test_cache(&root.join("inspect-me"));

    let config = test_config(root);

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(3)),
        method: "tools/call".into(),
        params: Some(serde_json::json!({
            "name": "context.inspect_cache",
            "arguments": {
                "cache": "inspect-me"
            }
        })),
    };

    let response = handlers::dispatch(&req, &config).await.unwrap();
    let result = response.result.unwrap();

    let text = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(parsed["cache_version"].as_str().unwrap().starts_with("sha256:"));
    assert_eq!(parsed["document_count"].as_u64().unwrap(), 2);
    assert_eq!(parsed["valid"].as_bool().unwrap(), true);
}
