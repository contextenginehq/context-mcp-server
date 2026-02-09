pub mod health;
pub mod inspect_cache;
pub mod list_caches;
pub mod resolve_context;

use crate::config::ServerConfig;
use crate::protocol::{
    InspectCacheParams, JsonRpcError, JsonRpcRequest, JsonRpcResponse, ResolveContextParams,
    ToolCallParams, ToolResult,
};

/// Dispatch a JSON-RPC request to the appropriate handler.
///
/// Returns `None` for notifications (no response required).
pub async fn dispatch(
    req: &JsonRpcRequest,
    config: &ServerConfig,
) -> Option<JsonRpcResponse> {
    match req.method.as_str() {
        "initialize" => {
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mcp-context-server",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });
            Some(JsonRpcResponse::success(req.id.clone(), result))
        }

        "notifications/initialized" => None,

        "ping" => Some(JsonRpcResponse::success(req.id.clone(), serde_json::json!({}))),

        "tools/list" => {
            let result = serde_json::json!({
                "tools": [
                    {
                        "name": "context.resolve",
                        "description": "Resolve context from a cache using a query and token budget",
                        "inputSchema": {
                            "type": "object",
                            "required": ["cache", "query", "budget"],
                            "properties": {
                                "cache": {
                                    "type": "string",
                                    "description": "Cache directory name (relative to CONTEXT_CACHE_ROOT)"
                                },
                                "query": {
                                    "type": "string",
                                    "description": "Search query for context selection"
                                },
                                "budget": {
                                    "type": "integer",
                                    "description": "Maximum token budget for selected context",
                                    "minimum": 0
                                }
                            }
                        }
                    },
                    {
                        "name": "context.list_caches",
                        "description": "List available context caches under the server's cache root",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "context.inspect_cache",
                        "description": "Inspect cache structure, metadata, and validity",
                        "inputSchema": {
                            "type": "object",
                            "required": ["cache"],
                            "properties": {
                                "cache": {
                                    "type": "string",
                                    "description": "Cache directory name (relative to CONTEXT_CACHE_ROOT)"
                                }
                            }
                        }
                    }
                ]
            });
            Some(JsonRpcResponse::success(req.id.clone(), result))
        }

        "tools/call" => {
            let params: ToolCallParams = match &req.params {
                Some(v) => match serde_json::from_value(v.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        return Some(JsonRpcResponse::error(
                            req.id.clone(),
                            JsonRpcError::invalid_params(format!(
                                "Invalid tools/call params: {e}"
                            )),
                        ));
                    }
                },
                None => {
                    return Some(JsonRpcResponse::error(
                        req.id.clone(),
                        JsonRpcError::invalid_params("Missing params for tools/call"),
                    ));
                }
            };

            let tool_result = dispatch_tool_call(&params, config).await;
            let result_json = serde_json::to_value(&tool_result).expect("ToolResult must serialize to JSON Value");
            Some(JsonRpcResponse::success(req.id.clone(), result_json))
        }

        _ => Some(JsonRpcResponse::error(
            req.id.clone(),
            JsonRpcError::method_not_found(&req.method),
        )),
    }
}

async fn dispatch_tool_call(params: &ToolCallParams, config: &ServerConfig) -> ToolResult {
    match params.name.as_str() {
        "context.resolve" => {
            let resolve_params: ResolveContextParams = match &params.arguments {
                Some(v) => match serde_json::from_value(v.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Invalid arguments for context.resolve: {e}"
                        ));
                    }
                },
                None => {
                    return ToolResult::error(
                        "Missing arguments for context.resolve",
                    );
                }
            };
            resolve_context::handle(resolve_params, config).await
        }

        "context.list_caches" => list_caches::handle(config).await,

        "context.inspect_cache" => {
            let inspect_params: InspectCacheParams = match &params.arguments {
                Some(v) => match serde_json::from_value(v.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Invalid arguments for context.inspect_cache: {e}"
                        ));
                    }
                },
                None => {
                    return ToolResult::error(
                        "Missing arguments for context.inspect_cache",
                    );
                }
            };
            inspect_cache::handle(inspect_params, config).await
        }

        "health" => health::handle().await,

        _ => ToolResult::error(format!("Unknown tool: {}", params.name)),
    }
}
