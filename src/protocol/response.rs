use serde::{Deserialize, Serialize};

use super::request::RpcId;

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 response layer
// ---------------------------------------------------------------------------

/// JSON-RPC 2.0 response envelope.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<RpcId>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<RpcId>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// JSON-RPC 2.0 error object (protocol-level errors).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    pub fn parse_error() -> Self {
        Self { code: -32700, message: "Parse error".into(), data: None }
    }

    pub fn invalid_request() -> Self {
        Self { code: -32600, message: "Invalid Request".into(), data: None }
    }

    pub fn invalid_request_with(detail: impl Into<String>) -> Self {
        Self { code: -32600, message: detail.into(), data: None }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self { code: -32602, message: detail.into(), data: None }
    }

    pub fn internal_error(detail: impl Into<String>) -> Self {
        Self { code: -32603, message: detail.into(), data: None }
    }
}

// ---------------------------------------------------------------------------
// MCP tool result layer (returned inside a *successful* JSON-RPC response)
// ---------------------------------------------------------------------------

/// MCP tool call result wrapper.
#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    pub content: Vec<ToolResultContent>,
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

/// A single content block inside a tool result.
#[derive(Debug, Clone, Serialize)]
pub struct ToolResultContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent {
                content_type: "text".into(),
                text: text.into(),
            }],
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent {
                content_type: "text".into(),
                text: text.into(),
            }],
            is_error: true,
        }
    }
}

// ---------------------------------------------------------------------------
// MCP domain-level error types (migrated from mcp/error.rs)
// ---------------------------------------------------------------------------

/// MCP error code (v0)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpErrorCode {
    CacheMissing,
    CacheInvalid,
    InvalidQuery,
    InvalidBudget,
    IoError,
    InternalError,
}

impl McpErrorCode {
    /// Map to the corresponding JSON-RPC 2.0 error code.
    ///
    /// Input validation failures → -32602 (Invalid params)
    /// Server-side failures     → -32603 (Internal error)
    pub fn json_rpc_code(&self) -> i32 {
        match self {
            Self::CacheMissing | Self::CacheInvalid => -32602,
            Self::InvalidQuery | Self::InvalidBudget => -32602,
            Self::IoError | Self::InternalError => -32603,
        }
    }
}

/// MCP error object
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpError {
    pub code: McpErrorCode,
    pub message: String,
}

/// MCP error response (top-level)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpErrorResponse {
    pub error: McpError,
}

impl McpErrorResponse {
    pub fn new(code: McpErrorCode, message: impl Into<String>) -> Self {
        Self {
            error: McpError {
                code,
                message: message.into(),
            },
        }
    }

    /// Construct with the spec-recommended canonical message for a given code.
    ///
    /// Messages match error_schema.md "Recommended canonical messages" exactly.
    pub fn canonical(code: McpErrorCode) -> Self {
        let message = match &code {
            McpErrorCode::CacheMissing => "Cache does not exist",
            McpErrorCode::CacheInvalid => "Cache exists but is invalid",
            McpErrorCode::InvalidQuery => "Query is invalid",
            McpErrorCode::InvalidBudget => "Budget is invalid",
            McpErrorCode::IoError => "I/O error occurred",
            McpErrorCode::InternalError => "Internal error",
        };
        Self::new(code, message)
    }
}

/// Convert an MCP domain error into a JSON-RPC error.
///
/// The JSON-RPC `code` is derived from the MCP error code.
/// The JSON-RPC `message` is the human-readable MCP message.
/// The full MCP error object is carried in `data` for structured clients.
impl From<McpErrorResponse> for JsonRpcError {
    fn from(mcp: McpErrorResponse) -> Self {
        Self {
            code: mcp.error.code.json_rpc_code(),
            message: mcp.error.message.clone(),
            data: Some(serde_json::to_value(&mcp).expect("McpErrorResponse must serialize to JSON Value")),
        }
    }
}

/// Convert an MCP domain error into a tool result with `isError: true`.
///
/// The text content is the JSON-serialized `McpErrorResponse`, preserving
/// the structured error for clients that inspect tool output.
impl From<McpErrorResponse> for ToolResult {
    fn from(mcp: McpErrorResponse) -> Self {
        let json = serde_json::to_string(&mcp).expect("McpErrorResponse must serialize to JSON string");
        Self::error(format!("{json}\n"))
    }
}
