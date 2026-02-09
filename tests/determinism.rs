//! Determinism regression test.
//!
//! Validates the central invariant from INVARIANT.md:
//!   f(documents, config, query) → deterministic output
//!
//! For identical cache contents, query, and budget, the JSON output
//! from context selection MUST be byte-identical across:
//! - multiple runs
//! - cache rebuilds

use std::path::Path;

use context_core::cache::{CacheBuildConfig, CacheBuilder, ContextCache, CacheManifest};
use context_core::document::{Document, DocumentId, Metadata};
use context_core::selection::ContextSelector;
use context_core::types::Query;

/// Build fixture documents with known, stable content.
fn fixture_documents(root: &Path) -> Vec<Document> {
    let docs = vec![
        ("docs/api.md", "API reference for the context platform REST endpoints and authentication"),
        ("docs/deployment.md", "Deployment guide for production environments including Docker and Kubernetes"),
        ("docs/architecture.md", "System architecture overview describing the cache compiler pipeline"),
        ("docs/quickstart.md", "Getting started with context resolve in five minutes"),
    ];

    docs.into_iter()
        .map(|(rel_path, content)| {
            let source = root.join(rel_path);
            let id = DocumentId::from_path(root, &source).unwrap();
            Document::ingest(
                id,
                rel_path.to_string(),
                content.as_bytes().to_vec(),
                Metadata::new(),
            )
            .unwrap()
        })
        .collect()
}

/// Build a cache from fixture documents into `output_dir`.
fn build_cache(output_dir: &Path, root: &Path) -> ContextCache {
    let docs = fixture_documents(root);
    let builder = CacheBuilder::new(CacheBuildConfig::v0());
    builder.build(docs, output_dir).unwrap()
}

/// Load a cache from disk (simulates what the MCP handler does).
fn load_cache(cache_dir: &Path) -> ContextCache {
    let manifest_path = cache_dir.join("manifest.json");
    let f = std::fs::File::open(&manifest_path).unwrap();
    let manifest: CacheManifest = serde_json::from_reader(f).unwrap();
    ContextCache {
        root: cache_dir.to_path_buf(),
        manifest,
    }
}

/// Run selection and serialize to JSON (same path as the MCP handler).
fn select_to_json(cache: &ContextCache, query: &str, budget: usize) -> String {
    let selector = ContextSelector::default();
    let q = Query::new(query);
    let result = selector.select(cache, q, budget).unwrap();
    serde_json::to_string(&result).unwrap()
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[test]
fn identical_runs_produce_identical_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("source");
    std::fs::create_dir_all(&root).unwrap();

    let cache_dir = tmp.path().join("cache");
    let cache = build_cache(&cache_dir, &root);

    let run_a = select_to_json(&cache, "deployment", 4096);
    let run_b = select_to_json(&cache, "deployment", 4096);

    assert_eq!(
        run_a, run_b,
        "Two runs with identical inputs must produce byte-identical output"
    );
}

#[test]
fn rebuild_produces_identical_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("source");
    std::fs::create_dir_all(&root).unwrap();

    // Build #1
    let cache_dir_1 = tmp.path().join("cache1");
    let cache_1 = build_cache(&cache_dir_1, &root);
    let output_1 = select_to_json(&cache_1, "deployment", 4096);

    // Build #2 — same documents, fresh cache directory
    let cache_dir_2 = tmp.path().join("cache2");
    let cache_2 = build_cache(&cache_dir_2, &root);
    let output_2 = select_to_json(&cache_2, "deployment", 4096);

    assert_eq!(
        output_1, output_2,
        "Rebuild from identical documents must produce byte-identical selection output"
    );
}

#[test]
fn reload_from_disk_produces_identical_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("source");
    std::fs::create_dir_all(&root).unwrap();

    let cache_dir = tmp.path().join("cache");
    let cache_built = build_cache(&cache_dir, &root);

    // Select from the in-memory cache (returned by builder)
    let from_memory = select_to_json(&cache_built, "architecture", 4096);

    // Select from a cache loaded from disk (what the MCP handler does)
    let cache_loaded = load_cache(&cache_dir);
    let from_disk = select_to_json(&cache_loaded, "architecture", 4096);

    assert_eq!(
        from_memory, from_disk,
        "Selection from in-memory cache and disk-loaded cache must be byte-identical"
    );
}

#[test]
fn multiple_queries_are_each_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("source");
    std::fs::create_dir_all(&root).unwrap();

    let cache_dir = tmp.path().join("cache");
    let cache = build_cache(&cache_dir, &root);

    let cases = vec![
        ("deployment", 4096),
        ("api authentication", 2048),
        ("architecture pipeline", 1024),
        ("quickstart", 512),
        ("nonexistent topic", 4096),
        ("deployment", 0),
    ];

    for (query, budget) in &cases {
        let a = select_to_json(&cache, query, *budget);
        let b = select_to_json(&cache, query, *budget);
        assert_eq!(
            a, b,
            "Query {:?} budget {} must produce byte-identical output across runs",
            query, budget
        );
    }
}

#[test]
fn output_is_valid_json_with_expected_structure() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("source");
    std::fs::create_dir_all(&root).unwrap();

    let cache_dir = tmp.path().join("cache");
    let cache = build_cache(&cache_dir, &root);

    let json_str = select_to_json(&cache, "deployment", 4096);
    let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Must have top-level "documents" and "selection" keys
    assert!(value.get("documents").is_some(), "Missing 'documents' key");
    assert!(value.get("selection").is_some(), "Missing 'selection' key");

    let selection = value.get("selection").unwrap();
    assert_eq!(selection.get("query").unwrap().as_str().unwrap(), "deployment");
    assert_eq!(selection.get("budget").unwrap().as_u64().unwrap(), 4096);
    assert!(selection.get("tokens_used").is_some());
    assert!(selection.get("documents_considered").is_some());
    assert!(selection.get("documents_selected").is_some());
    assert!(selection.get("documents_excluded_by_budget").is_some());

    // Each document must have the required fields
    let docs = value.get("documents").unwrap().as_array().unwrap();
    for doc in docs {
        assert!(doc.get("id").is_some(), "Document missing 'id'");
        assert!(doc.get("version").is_some(), "Document missing 'version'");
        assert!(doc.get("content").is_some(), "Document missing 'content'");
        assert!(doc.get("score").is_some(), "Document missing 'score'");
        assert!(doc.get("tokens").is_some(), "Document missing 'tokens'");
        assert!(doc.get("why").is_some(), "Document missing 'why'");
    }
}

#[test]
fn zero_budget_returns_empty_documents_deterministically() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("source");
    std::fs::create_dir_all(&root).unwrap();

    let cache_dir = tmp.path().join("cache");
    let cache = build_cache(&cache_dir, &root);

    let a = select_to_json(&cache, "deployment", 0);
    let b = select_to_json(&cache, "deployment", 0);

    assert_eq!(a, b, "Zero budget must produce byte-identical output");

    let value: serde_json::Value = serde_json::from_str(&a).unwrap();
    let docs = value.get("documents").unwrap().as_array().unwrap();
    assert!(docs.is_empty(), "Zero budget should select no documents");
}
