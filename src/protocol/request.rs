use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 ID — may be a number or string per spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    Number(i64),
    Str(String),
}

/// JSON-RPC 2.0 request envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<RpcId>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// Parameters for the `context.resolve` tool.
#[derive(Debug, Clone, Deserialize)]
pub struct ResolveContextParams {
    pub cache: String,
    pub query: String,
    /// Accepts i64 so we can detect negative values before casting to usize.
    pub budget: i64,
}

/// Parameters for the `context.list_caches` tool.
#[derive(Debug, Clone, Deserialize)]
pub struct ListCachesParams {
    pub root: String,
}

/// Parameters for the `context.inspect_cache` tool.
#[derive(Debug, Clone, Deserialize)]
pub struct InspectCacheParams {
    pub cache: String,
}

/// MCP `initialize` params.
#[derive(Debug, Clone, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: Option<String>,
    #[serde(rename = "clientInfo")]
    pub client_info: Option<ClientInfo>,
}

/// Client information sent during `initialize`.
#[derive(Debug, Clone, Deserialize)]
pub struct ClientInfo {
    pub name: Option<String>,
    pub version: Option<String>,
}

/// Parameters for `tools/call`.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    pub arguments: Option<serde_json::Value>,
}
