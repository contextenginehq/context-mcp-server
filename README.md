# mcp-context-server

[![Crates.io](https://img.shields.io/crates/v/mcp-context-server.svg)](https://crates.io/crates/mcp-context-server)
[![Docs.rs](https://docs.rs/mcp-context-server/badge.svg)](https://docs.rs/mcp-context-server)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

MCP server for the Context platform.

`mcp-context-server` exposes context resolution to AI agents via the [Model Context Protocol](https://modelcontextprotocol.io/) over stdio (JSON-RPC 2.0, newline-delimited). It is the primary integration surface for agents.

## Tools

| Tool | Description |
|------|-------------|
| `context.resolve` | Resolve context from a cache using a query and token budget |
| `context.list_caches` | List available context caches under the server's cache root |
| `context.inspect_cache` | Inspect cache metadata and validity |

## Configuration

The server is configured via environment variables:

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `CONTEXT_CACHE_ROOT` | yes | — | Root directory containing context caches |
| `CONTEXT_TOOL_TIMEOUT_SECS` | no | 30 | Maximum seconds per tool call |

## Running

```bash
CONTEXT_CACHE_ROOT=./caches ./dist/mcp-context-server
```

The server reads JSON-RPC requests from stdin and writes responses to stdout. It is designed to be launched by an MCP client (e.g., Claude Desktop, an agent framework).

## MCP client configuration

For Claude Desktop, add to your MCP config:

```json
{
  "mcpServers": {
    "context": {
      "command": "/path/to/mcp-context-server",
      "env": {
        "CONTEXT_CACHE_ROOT": "/path/to/your/caches"
      }
    }
  }
}
```

## Protocol

- Transport: stdio (JSON-RPC 2.0, newline-delimited)
- Protocol version: `2024-11-05`
- All responses are deterministic
- Error codes: `cache_missing`, `cache_invalid`, `invalid_query`, `invalid_budget`, `io_error`, `internal_error`

## Build

```bash
make build     # debug build
make test      # run all tests (31 tests including 11 golden snapshot tests)
make check     # cargo check + clippy
make release   # optimized build, binary copied to dist/
make clean     # remove artifacts
```

The release binary is named `mcp-context-server` and placed in `dist/`.

## Spec references

See `spec_refs.md` for links to the governing specifications.

---

"Context Engine" is a trademark of Context Engine Contributors. The software is open source under the [Apache License 2.0](LICENSE). The trademark is not licensed for use by third parties to market competing products or services without prior written permission.
