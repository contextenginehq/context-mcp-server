use crate::protocol::ToolResult;

/// Stub: health check.
pub async fn handle() -> ToolResult {
    ToolResult::text(r#"{"status":"ok"}"#)
}
