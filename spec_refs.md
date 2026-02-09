# Specification References

This crate implements the following specifications:

## JSON-RPC 2.0
- Spec: https://www.jsonrpc.org/specification
- Used for: Transport protocol over stdio (newline-delimited)
- Error codes: -32700 (parse), -32600 (invalid request), -32601 (method not found), -32602 (invalid params), -32603 (internal error)
- The `jsonrpc` field is validated to be exactly `"2.0"` on every request

## Model Context Protocol (MCP)
- Spec: https://spec.modelcontextprotocol.io/
- Protocol version: 2024-11-05
- Transport: stdio
- Capabilities: tools

### Supported methods
- `initialize` — handshake, returns server capabilities and tool list
- `notifications/initialized` — client acknowledgment (no response)
- `ping` — keep-alive, returns `{}`
- `tools/list` — enumerate available tools
- `tools/call` — invoke a tool by name

### Initialization gate
Requests other than `initialize` are rejected with `-32600` until the handshake completes.

## context.resolve Tool
- Spec: `context-specs/core/mcp/context.resolve.md`
- Backed by `context-core` crate
- Input: `cache` (string), `query` (string), `budget` (integer, minimum 0)
- Output: `SelectionResult` from context-core (documents + selection metadata)
- Domain errors use `McpErrorResponse` with `isError: true` in the tool result
- Error messages use canonical text only — no paths, stack traces, or OS errors (per error_schema.md)

## MCP Error Schema (v0)
- Spec: `context-specs/core/mcp/error_schema.md`
- Frozen schema: `error.code` (enum string) + `error.message` (non-empty string)
- MCP error codes map to JSON-RPC codes via `McpErrorCode::json_rpc_code()`
- `McpErrorResponse` converts to both `JsonRpcError` (protocol layer) and `ToolResult` (tool layer)

## Server Hardening

### Message size limit
Incoming messages are rejected if they exceed 1 MiB (returns parse error).

### Tool timeout
Blocking operations (`spawn_blocking`) are wrapped in a configurable timeout (default 30s, `CONTEXT_TOOL_TIMEOUT_SECS`). Timeout returns `internal_error`.

### Path traversal protection
Cache names are validated before filesystem access:
- `..`, leading `/`, leading `\` are rejected before touching disk
- Both candidate and root are canonicalized (resolves symlinks)
- Canonical path must remain inside the canonical cache root
