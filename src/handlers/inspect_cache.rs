use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::ServerConfig;
use crate::protocol::{InspectCacheParams, McpErrorCode, McpErrorResponse, ToolResult};

#[derive(Debug, Serialize)]
struct InspectCacheResponse {
    cache_version: String,
    document_count: usize,
    total_bytes: u64,
    valid: bool,
}

/// Handle a `context.inspect_cache` tool call.
///
/// Resolves the cache name against the configured cache root (with
/// traversal protection), loads the manifest, and returns structural
/// metadata. Does not expose document content.
pub async fn handle(params: InspectCacheParams, config: &ServerConfig) -> ToolResult {
    let cache_path = match resolve_cache_path(&config.cache_root, &params.cache) {
        Ok(p) => p,
        Err(err) => return err.into(),
    };

    match inspect(&cache_path) {
        Ok(json) => ToolResult::text(json),
        Err(mcp_err) => mcp_err.into(),
    }
}

fn inspect(cache_path: &Path) -> Result<String, McpErrorResponse> {
    let manifest_path = cache_path.join("manifest.json");

    let mut valid = true;
    let mut cache_version = String::new();
    let mut document_count = 0usize;

    match std::fs::File::open(&manifest_path) {
        Ok(file) => match serde_json::from_reader::<_, serde_json::Value>(file) {
            Ok(value) => {
                if let Some(version) = value.get("cache_version").and_then(|v| v.as_str()) {
                    cache_version = version.to_string();
                } else {
                    valid = false;
                }

                if let Some(count) = value.get("document_count").and_then(|v| v.as_u64()) {
                    document_count = count as usize;
                } else {
                    valid = false;
                }
            }
            Err(_) => {
                valid = false;
            }
        },
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                valid = false;
            } else {
                eprintln!("Cannot read manifest: {err}");
                return Err(McpErrorResponse::canonical(McpErrorCode::IoError));
            }
        }
    }

    let total_bytes = if valid {
        match total_bytes_non_recursive(cache_path) {
            Ok(val) => val,
            Err(e) => {
                eprintln!("Error computing total_bytes: {e}");
                valid = false;
                0
            }
        }
    } else {
        0
    };

    let payload = InspectCacheResponse {
        cache_version,
        document_count,
        total_bytes,
        valid,
    };

    serde_json::to_string(&payload).map_err(|e| {
        eprintln!("Serialization failed: {e}");
        McpErrorResponse::canonical(McpErrorCode::InternalError)
    })
}

/// Sum file sizes in the cache directory (non-recursive, no symlinks).
fn total_bytes_non_recursive(root: &Path) -> Result<u64, std::io::Error> {
    let entries = std::fs::read_dir(root)?;
    let mut total = 0u64;

    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_file() {
            let metadata = entry.metadata()?;
            total += metadata.len();
        }
    }

    Ok(total)
}

/// Resolve and validate a cache path, preventing directory traversal.
///
/// Same protection as resolve_context: rejects `..`, absolute paths,
/// canonicalizes both paths, verifies containment.
fn resolve_cache_path(cache_root: &Path, cache_name: &str) -> Result<PathBuf, McpErrorResponse> {
    if cache_name.contains("..") || cache_name.starts_with('/') || cache_name.starts_with('\\') {
        return Err(McpErrorResponse::canonical(McpErrorCode::CacheMissing));
    }

    let candidate = cache_root.join(cache_name);

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
