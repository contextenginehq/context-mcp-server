use std::fs;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use context_core::cache::{CacheBuildConfig, CacheBuilder, CacheManifest, ContextCache};
use context_core::document::{Document, DocumentId, Metadata};
use context_core::selection::ContextSelector;
use context_core::types::Query;
use mcp_context_server::config::ServerConfig;
use mcp_context_server::handlers;
use mcp_context_server::protocol::{JsonRpcRequest, RpcId};
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/fixtures")
}

fn expected_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/expected")
        .join(name)
}

fn read_expected(name: &str) -> String {
    fs::read_to_string(expected_path(name))
        .expect("expected file missing")
        .trim_end()
        .to_string()
}

fn make_doc_from_file(root: &Path, file_name: &str) -> Document {
    let path = root.join(file_name);
    let content = fs::read(&path).expect("fixture doc missing");
    let id = DocumentId::from_path(root, &path).expect("invalid doc path");
    Document::ingest(id, file_name.to_string(), content, Metadata::default()).unwrap()
}

fn hash_dir_bytes(root: &Path) -> Vec<u8> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files);
    files.sort();

    let mut hasher = Sha256::new();
    for rel in files {
        // Skip manifest.json — it contains a non-deterministic created_at
        // timestamp. Manifest content is compared separately with normalization.
        if rel.to_string_lossy() == "manifest.json" {
            continue;
        }
        let path = root.join(&rel);
        let bytes = fs::read(&path).expect("failed to read file for hash");
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(&bytes);
    }

    hasher.finalize().to_vec()
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(root, &path, files);
            } else if path.is_file() {
                if let Ok(rel) = path.strip_prefix(root) {
                    files.push(rel.to_path_buf());
                }
            }
        }
    }
}

#[test]
fn golden_cache_build_determinism() {
    let docs_root = fixtures_root().join("docs");
    let docs = vec![
        make_doc_from_file(&docs_root, "deployment.md"),
        make_doc_from_file(&docs_root, "security.md"),
        make_doc_from_file(&docs_root, "empty.md"),
    ];

    let config = CacheBuildConfig::v0();
    let builder = CacheBuilder::new(config);

    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let cache_path1 = dir1.path().join("cache");
    let cache_path2 = dir2.path().join("cache");

    let cache1 = builder.build(docs.clone(), &cache_path1).unwrap();
    let cache2 = builder.build(docs, &cache_path2).unwrap();

    assert_eq!(cache1.manifest.cache_version, cache2.manifest.cache_version);

    let manifest_bytes_1 = fs::read(cache_path1.join("manifest.json")).unwrap();
    let manifest_bytes_2 = fs::read(cache_path2.join("manifest.json")).unwrap();

    let mut manifest_1: CacheManifest = serde_json::from_slice(&manifest_bytes_1).unwrap();
    let mut manifest_2: CacheManifest = serde_json::from_slice(&manifest_bytes_2).unwrap();
    let fixed_time = Utc.timestamp_opt(0, 0).unwrap();
    manifest_1.created_at = fixed_time;
    manifest_2.created_at = fixed_time;

    let normalized_manifest_1 = serde_json::to_string_pretty(&manifest_1).unwrap();
    let normalized_manifest_2 = serde_json::to_string_pretty(&manifest_2).unwrap();
    assert_eq!(normalized_manifest_1, normalized_manifest_2, "manifest.json mismatch");

    let expected_manifest = read_expected("build_manifest.json");
    assert_eq!(normalized_manifest_1, expected_manifest, "manifest does not match golden");

    let index_bytes_1 = fs::read(cache_path1.join("index.json")).unwrap();
    let index_bytes_2 = fs::read(cache_path2.join("index.json")).unwrap();
    assert_eq!(index_bytes_1, index_bytes_2, "index.json mismatch");

    for entry in &cache1.manifest.documents {
        let doc_bytes_1 = fs::read(cache_path1.join(&entry.file)).unwrap();
        let doc_bytes_2 = fs::read(cache_path2.join(&entry.file)).unwrap();
        assert_eq!(doc_bytes_1, doc_bytes_2, "document file mismatch: {}", entry.file);
    }

    let hash1 = hash_dir_bytes(&cache_path1);
    let hash2 = hash_dir_bytes(&cache_path2);
    assert_eq!(hash1, hash2, "cache directory hash mismatch");
}

#[test]
fn golden_selection_output() {
    let cache_root = fixtures_root().join("cache_valid");
    let manifest_path = cache_root.join("manifest.json");
    let manifest_file = fs::File::open(&manifest_path).unwrap();
    let manifest: CacheManifest = serde_json::from_reader(manifest_file).unwrap();

    let cache = ContextCache {
        root: cache_root,
        manifest,
    };

    let selector = ContextSelector::default();
    let query = Query::new("deployment");

    let result1 = selector.select(&cache, query.clone(), 4000).unwrap();
    let result2 = selector.select(&cache, query, 4000).unwrap();

    let json1 = serde_json::to_string(&result1).unwrap();
    let json2 = serde_json::to_string(&result2).unwrap();

    assert_eq!(json1, json2, "selection output is not deterministic");

    let expected = read_expected("resolve_basic.json");
    assert_eq!(json1, expected, "selection output does not match golden");
}

#[test]
fn golden_zero_budget_output() {
    let docs_root = fixtures_root().join("docs");
    let docs = vec![
        make_doc_from_file(&docs_root, "deployment.md"),
        make_doc_from_file(&docs_root, "security.md"),
    ];

    let builder = CacheBuilder::new(CacheBuildConfig::v0());
    let dir = tempdir().unwrap();
    let cache_path = dir.path().join("cache");

    let cache = builder.build(docs, &cache_path).unwrap();

    let selector = ContextSelector::default();
    let query = Query::new("deployment");
    let result = selector.select(&cache, query, 0).unwrap();

    let json = serde_json::to_string(&result).unwrap();
    let expected = read_expected("resolve_zero_budget.json");
    assert_eq!(json, expected, "zero budget output does not match golden");
}

#[test]
fn golden_ordering_output() {
    let root = Path::new("/root");
    let docs = vec![
        Document::ingest(
            DocumentId::from_path(root, &root.join("a.md")).unwrap(),
            "a.md".to_string(),
            b"apple 1".to_vec(),
            Metadata::default(),
        )
        .unwrap(),
        Document::ingest(
            DocumentId::from_path(root, &root.join("z.md")).unwrap(),
            "z.md".to_string(),
            b"apple 2".to_vec(),
            Metadata::default(),
        )
        .unwrap(),
    ];

    let builder = CacheBuilder::new(CacheBuildConfig::v0());
    let dir = tempdir().unwrap();
    let cache_path = dir.path().join("cache");
    let cache = builder.build(docs, &cache_path).unwrap();

    let selector = ContextSelector::default();
    let query = Query::new("apple");
    let result = selector.select(&cache, query, 100).unwrap();

    let json = serde_json::to_string(&result).unwrap();
    let expected = read_expected("resolve_ordering.json");
    assert_eq!(json, expected, "ordering output does not match golden");
}

#[tokio::test]
async fn golden_mcp_success_response() {
    let config = ServerConfig {
        cache_root: fixtures_root(),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({
            "name": "context.resolve",
            "arguments": {
                "cache": "cache_valid",
                "query": "deployment",
                "budget": 4000
            }
        })),
    };

    let resp = handlers::dispatch(&req, &config).await.expect("missing response");
    let json = serde_json::to_string(&resp).unwrap();

    let expected = read_expected("mcp_resolve_basic.json");
    assert_eq!(json, expected, "MCP resolve response does not match golden");
}

#[tokio::test]
async fn golden_mcp_error_cache_missing() {
    let config = ServerConfig {
        cache_root: fixtures_root(),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({
            "name": "context.inspect_cache",
            "arguments": {
                "cache": "./tests/golden/fixtures/does-not-exist"
            }
        })),
    };

    let resp = handlers::dispatch(&req, &config).await.expect("missing response");
    let json = serde_json::to_string(&resp).unwrap();

    let expected = read_expected("error_cache_missing.json");
    assert_eq!(json, expected, "cache_missing error response does not match golden");
}

#[tokio::test]
async fn golden_mcp_error_cache_invalid() {
    let config = ServerConfig {
        cache_root: fixtures_root(),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({
            "name": "context.resolve",
            "arguments": {
                "cache": "cache_corrupt",
                "query": "deployment",
                "budget": 4000
            }
        })),
    };

    let resp = handlers::dispatch(&req, &config).await.expect("missing response");
    let json = serde_json::to_string(&resp).unwrap();

    let expected = read_expected("error_cache_invalid.json");
    assert_eq!(json, expected, "cache_invalid error response does not match golden");
}

#[tokio::test]
async fn golden_list_caches_output() {
    let config = ServerConfig {
        cache_root: fixtures_root().join("cache_root"),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let result = handlers::list_caches::handle(&config).await;

    let output = &result.content[0].text;
    let expected = read_expected("list_caches.json");
    assert_eq!(output, &expected, "list_caches output does not match golden");
}

#[tokio::test]
async fn golden_inspect_cache_outputs() {
    let config = ServerConfig {
        cache_root: fixtures_root(),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let valid_result = handlers::inspect_cache::handle(
        mcp_context_server::protocol::InspectCacheParams {
            cache: "cache_valid".to_string(),
        },
        &config,
    )
    .await;

    let invalid_result = handlers::inspect_cache::handle(
        mcp_context_server::protocol::InspectCacheParams {
            cache: "cache_corrupt".to_string(),
        },
        &config,
    )
    .await;

    let expected_valid = read_expected("inspect_valid.json");
    let expected_invalid = read_expected("inspect_invalid.json");

    assert_eq!(valid_result.content[0].text, expected_valid, "inspect valid output mismatch");
    assert_eq!(invalid_result.content[0].text, expected_invalid, "inspect invalid output mismatch");
}

#[tokio::test]
async fn golden_end_to_end_determinism() {
    let docs_root = fixtures_root().join("docs");
    let docs = vec![
        make_doc_from_file(&docs_root, "deployment.md"),
        make_doc_from_file(&docs_root, "security.md"),
        make_doc_from_file(&docs_root, "empty.md"),
    ];

    let builder = CacheBuilder::new(CacheBuildConfig::v0());

    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let cache_root1 = dir1.path();
    let cache_root2 = dir2.path();

    let cache_path1 = cache_root1.join("cache");
    let cache_path2 = cache_root2.join("cache");

    let _cache1 = builder.build(docs.clone(), &cache_path1).unwrap();
    let _cache2 = builder.build(docs, &cache_path2).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({
            "name": "context.resolve",
            "arguments": {
                "cache": "cache",
                "query": "deployment",
                "budget": 4000
            }
        })),
    };

    let config1 = ServerConfig {
        cache_root: cache_root1.to_path_buf(),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let config2 = ServerConfig {
        cache_root: cache_root2.to_path_buf(),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let resp1 = handlers::dispatch(&req, &config1).await.expect("missing response");
    let resp2 = handlers::dispatch(&req, &config2).await.expect("missing response");

    let json1 = serde_json::to_string(&resp1).unwrap();
    let json2 = serde_json::to_string(&resp2).unwrap();

    assert_eq!(json1, json2, "end-to-end pipeline is not deterministic");
}

#[cfg(unix)]
#[tokio::test]
async fn golden_permission_denied_io_error() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let cache_dir = dir.path().join("locked_cache");
    fs::create_dir(&cache_dir).unwrap();

    let manifest_path = cache_dir.join("manifest.json");
    fs::write(&manifest_path, r#"{"cache_version":"v1","document_count":0}"#).unwrap();

    // Make manifest unreadable to trigger PermissionDenied
    fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let config = ServerConfig {
        cache_root: dir.path().to_path_buf(),
        tool_timeout: std::time::Duration::from_secs(5),
    };

    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(RpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({
            "name": "context.inspect_cache",
            "arguments": {
                "cache": "locked_cache"
            }
        })),
    };

    let resp = handlers::dispatch(&req, &config).await.expect("missing response");
    let json = serde_json::to_string(&resp).unwrap();

    // Restore permissions so tempdir cleanup can delete the file
    fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let expected = read_expected("error_permission_denied.json");
    assert_eq!(json, expected, "permission denied error response does not match golden");
}
