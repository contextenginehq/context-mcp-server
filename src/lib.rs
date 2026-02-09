//! MCP server for the Context Engine.
//!
//! Exposes `context.resolve`, `context.list_caches`, and `context.inspect_cache`
//! tools over JSON-RPC 2.0 stdio transport, compatible with any MCP-aware AI agent.
//!
//! See <https://github.com/contextenginehq/context-engine> for the full platform.

pub mod config;
pub mod handlers;
pub mod protocol;
pub mod server;

pub mod schema;
