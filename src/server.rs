use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::ServerConfig;
use crate::handlers;
use crate::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

/// Maximum bytes per JSON-RPC message (1 MiB).
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// MCP server that communicates over stdio using newline-delimited JSON-RPC 2.0.
pub struct McpServer {
    config: ServerConfig,
    initialized: bool,
}

impl McpServer {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            initialized: false,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut raw = Vec::new();

        loop {
            raw.clear();
            let n = reader.read_until(b'\n', &mut raw).await?;
            if n == 0 {
                break;
            }

            if n > MAX_MESSAGE_BYTES {
                eprintln!("Message too large: {n} bytes (limit {MAX_MESSAGE_BYTES})");
                write_response(
                    &mut stdout,
                    &JsonRpcResponse::error(None, JsonRpcError::parse_error()),
                ).await?;
                continue;
            }

            let trimmed = match std::str::from_utf8(&raw) {
                Ok(s) => s.trim(),
                Err(_) => {
                    write_response(
                        &mut stdout,
                        &JsonRpcResponse::error(None, JsonRpcError::parse_error()),
                    ).await?;
                    continue;
                }
            };

            if trimmed.is_empty() {
                continue;
            }

            let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Parse error: {e}");
                    write_response(
                        &mut stdout,
                        &JsonRpcResponse::error(None, JsonRpcError::parse_error()),
                    ).await?;
                    continue;
                }
            };

            // Validate jsonrpc version
            if req.jsonrpc != "2.0" {
                write_response(
                    &mut stdout,
                    &JsonRpcResponse::error(req.id.clone(), JsonRpcError::invalid_request()),
                ).await?;
                continue;
            }

            // Initialization gate: only `initialize` is allowed before handshake completes
            if !self.initialized && req.method != "initialize" {
                if req.id.is_none() {
                    continue;
                }
                write_response(
                    &mut stdout,
                    &JsonRpcResponse::error(
                        req.id.clone(),
                        JsonRpcError::invalid_request_with("Server not initialized"),
                    ),
                ).await?;
                continue;
            }

            if let Some(resp) = handlers::dispatch(&req, &self.config).await {
                write_response(&mut stdout, &resp).await?;
            }

            if req.method == "initialize" {
                self.initialized = true;
            }
        }

        Ok(())
    }
}

async fn write_response(
    stdout: &mut tokio::io::Stdout,
    resp: &JsonRpcResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = serde_json::to_string(resp)?;
    stdout.write_all(out.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}
