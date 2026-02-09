use std::path::Path;

use serde::Serialize;

use crate::config::ServerConfig;
use crate::protocol::{McpErrorCode, McpErrorResponse, ToolResult};

#[derive(Debug, Serialize)]
struct ListCachesResponse {
    caches: Vec<CacheEntry>,
}

#[derive(Debug, Serialize)]
struct CacheEntry {
    path: String,
    has_manifest: bool,
}

/// Handle a `context.list_caches` tool call.
///
/// Enumerates immediate subdirectories of the configured cache root.
/// For each subdirectory, checks whether `manifest.json` exists as a
/// regular file. No JSON parsing is performed — this is a discovery
/// tool, not a validation tool.
///
/// Results are sorted by path ascending (UTF-8 byte order) for determinism.
/// The server's configured cache root is used; no client-supplied root is
/// accepted (per mcp_interface.md: "No parameters required").
pub async fn handle(config: &ServerConfig) -> ToolResult {
    match enumerate_caches(&config.cache_root) {
        Ok(json) => ToolResult::text(json),
        Err(mcp_err) => mcp_err.into(),
    }
}

fn enumerate_caches(cache_root: &Path) -> Result<String, McpErrorResponse> {
    if !cache_root.is_dir() {
        return Err(McpErrorResponse::canonical(McpErrorCode::CacheMissing));
    }

    let entries = std::fs::read_dir(cache_root).map_err(|e| {
        eprintln!("Cannot read cache root: {e}");
        McpErrorResponse::canonical(McpErrorCode::IoError)
    })?;

    let mut caches = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| {
            eprintln!("Error reading directory entry: {e}");
            McpErrorResponse::canonical(McpErrorCode::IoError)
        })?;

        let file_type = entry.file_type().map_err(|e| {
            eprintln!("Cannot read file type: {e}");
            McpErrorResponse::canonical(McpErrorCode::IoError)
        })?;

        // Only immediate subdirectories — skip files and symlinks
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let manifest_path = entry.path().join("manifest.json");

        // Check manifest existence without following symlinks
        let has_manifest = match std::fs::symlink_metadata(&manifest_path) {
            Ok(meta) => meta.is_file(),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    false
                } else {
                    return Err(McpErrorResponse::canonical(McpErrorCode::IoError));
                }
            }
        };

        caches.push(CacheEntry {
            path: name,
            has_manifest,
        });
    }

    // Sort by path ascending (UTF-8 byte order) for determinism
    caches.sort_by(|a, b| a.path.cmp(&b.path));

    let payload = ListCachesResponse { caches };
    serde_json::to_string(&payload).map_err(|e| {
        eprintln!("Serialization failed: {e}");
        McpErrorResponse::canonical(McpErrorCode::InternalError)
    })
}
