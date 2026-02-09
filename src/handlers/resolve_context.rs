use std::path::{Path, PathBuf};

use context_core::cache::{CacheManifest, ContextCache};
use context_core::selection::ContextSelector;
use context_core::types::Query;

use crate::config::ServerConfig;
use crate::protocol::{McpErrorCode, McpErrorResponse, ResolveContextParams, ToolResult};

/// Handle a `context.resolve` tool call.
pub async fn handle(
    params: ResolveContextParams,
    config: &ServerConfig,
) -> ToolResult {
    // Validate budget
    if params.budget < 0 {
        return McpErrorResponse::canonical(McpErrorCode::InvalidBudget).into();
    }
    let budget = params.budget as usize;

    // Resolve cache path (with traversal protection)
    let cache_path = match resolve_cache_path(&config.cache_root, &params.cache) {
        Ok(p) => p,
        Err(err) => return err.into(),
    };

    // Load cache and run selection on a blocking thread (context-core is sync)
    let timeout = config.tool_timeout;
    let task = tokio::task::spawn_blocking(move || {
        load_and_select(&cache_path, &params.query, budget)
    });

    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(Ok(selection_json))) => ToolResult::text(selection_json),
        Ok(Ok(Err(mcp_err))) => mcp_err.into(),
        Ok(Err(join_err)) => {
            eprintln!("Task join error: {join_err}");
            McpErrorResponse::canonical(McpErrorCode::InternalError).into()
        }
        Err(_) => {
            eprintln!("Operation timed out after {} seconds", timeout.as_secs());
            McpErrorResponse::canonical(McpErrorCode::InternalError).into()
        }
    }
}

/// Synchronous cache load + selection (runs inside spawn_blocking).
fn load_and_select(
    cache_path: &Path,
    query_str: &str,
    budget: usize,
) -> Result<String, McpErrorResponse> {
    let manifest_path = cache_path.join("manifest.json");
    let manifest_file = std::fs::File::open(&manifest_path).map_err(|e| {
        eprintln!("Cannot read manifest: {e}");
        // OS-level failure (permission denied, disk error) → io_error
        // Missing file in a validated directory → cache_invalid (structural)
        if e.kind() == std::io::ErrorKind::NotFound {
            McpErrorResponse::canonical(McpErrorCode::CacheInvalid)
        } else {
            McpErrorResponse::canonical(McpErrorCode::IoError)
        }
    })?;
    let manifest: CacheManifest = serde_json::from_reader(manifest_file).map_err(|e| {
        eprintln!("Invalid manifest JSON: {e}");
        McpErrorResponse::canonical(McpErrorCode::CacheInvalid)
    })?;

    let cache = ContextCache {
        root: cache_path.to_path_buf(),
        manifest,
    };

    let selector = ContextSelector::default();
    let query = Query::new(query_str);

    let selection = selector.select(&cache, query, budget).map_err(|e| {
        eprintln!("Selection failed: {e}");
        McpErrorResponse::canonical(McpErrorCode::InternalError)
    })?;

    let json = serde_json::to_string(&selection).map_err(|e| {
        eprintln!("Serialization failed: {e}");
        McpErrorResponse::canonical(McpErrorCode::InternalError)
    })?;

    Ok(format!("{json}\n"))
}

/// Resolve and validate a cache path, preventing directory traversal.
///
/// Canonicalizes both the cache root and the joined path, then verifies the
/// result is still inside the root. Rejects `..` segments, absolute paths,
/// and symlinks that escape the root.
fn resolve_cache_path(cache_root: &Path, cache_name: &str) -> Result<PathBuf, McpErrorResponse> {
    // Reject obvious traversal attempts before touching the filesystem
    if cache_name.contains("..") || cache_name.starts_with('/') || cache_name.starts_with('\\') {
        return Err(McpErrorResponse::canonical(McpErrorCode::CacheMissing));
    }

    let candidate = cache_root.join(cache_name);

    // Canonicalize resolves symlinks and normalizes the path
    let canonical = candidate.canonicalize().map_err(|_| {
        McpErrorResponse::canonical(McpErrorCode::CacheMissing)
    })?;

    let root_canonical = cache_root.canonicalize().map_err(|e| {
        eprintln!("Cache root not accessible: {e}");
        McpErrorResponse::canonical(McpErrorCode::IoError)
    })?;

    if !canonical.starts_with(&root_canonical) {
        return Err(McpErrorResponse::canonical(McpErrorCode::CacheMissing));
    }

    if !canonical.is_dir() {
        return Err(McpErrorResponse::canonical(McpErrorCode::CacheMissing));
    }

    Ok(canonical)
}
