# mcp-context-server — Implementation Progress

## Status: v0 complete — all tools implemented, 20/20 tests pass

The server runs, handles the MCP lifecycle, and dispatches all three tools (`context.resolve`, `context.list_caches`, `context.inspect_cache`) to `context-core`. Hardening is done. Specs are aligned. Determinism is verified across all tools.

---

## Completed

### Infrastructure
- [x] Binary + lib targets in Cargo.toml
- [x] `src/main.rs` — entry point, loads config, runs server
- [x] `src/server.rs` — async stdio JSON-RPC loop (tokio BufReader)
- [x] `src/config.rs` — `ServerConfig` from env (`CONTEXT_CACHE_ROOT`, `CONTEXT_TOOL_TIMEOUT_SECS`)
- [x] `src/lib.rs` — module declarations

### Protocol types
- [x] `src/protocol/request.rs` — `JsonRpcRequest`, `RpcId`, `ResolveContextParams`, `InspectCacheParams`, `ToolCallParams`, `InitializeParams`
- [x] `src/protocol/response.rs` — `JsonRpcResponse`, `JsonRpcError`, `ToolResult`, `McpErrorCode/McpError/McpErrorResponse`
- [x] `McpErrorCode::json_rpc_code()` — maps domain errors to JSON-RPC codes
- [x] `From<McpErrorResponse>` for both `JsonRpcError` and `ToolResult`

### Handlers
- [x] `handlers/mod.rs` — dispatch: `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`
- [x] `handlers/resolve_context.rs` — full handler: validate params, resolve cache path, `spawn_blocking` → `context-core`, return `SelectionResult`
- [x] `handlers/list_caches.rs` — enumerates subdirectories of `cache_root`, checks `manifest.json` existence, sorted by path ascending
- [x] `handlers/inspect_cache.rs` — resolves cache path with traversal protection, loads manifest, returns `cache_version`/`document_count`/`total_bytes`/`valid`
- [x] `handlers/health.rs` — returns `{"status":"ok"}`
- [x] `tools/list` advertises all 3 tools with input schemas

### Hardening
- [x] `jsonrpc: "2.0"` validation on every request
- [x] Initialization gate — rejects pre-init requests with `-32600`
- [x] Path traversal protection — rejects `..`, canonicalizes, verifies containment (resolve_context + inspect_cache)
- [x] Message size limit — 1 MiB max per line, returns parse error
- [x] Tool timeout — configurable (default 30s), wraps `spawn_blocking`
- [x] Canonical error messages only — no paths/traces/OS errors leaked to clients
- [x] `list_caches` uses server-configured cache root — no client-supplied path

### Spec compliance
- [x] Canonical messages match `error_schema.md` recommended text exactly
- [x] `io_error` code used for OS-level failures; `cache_invalid` for structural failures
- [x] All error paths use `McpErrorResponse::canonical()` — diagnostics go to stderr only
- [x] Empty queries passed through to context-core (score 0.0 for all docs)
- [x] `list_caches` sorted by path ascending for determinism (per `context.list_caches.md`)
- [x] `inspect_cache` returns `valid: false` for corrupt manifests (per `context.inspect_cache.md`)

### Tests
- [x] `tests/mcp_error_schema.rs` — golden schema + snapshot validation (1 test)
- [x] `tests/schema_harness.rs` — JSON schema validation harness (1 test)
- [x] `tests/determinism.rs` — determinism regression suite (6 tests)
- [x] `tests/handler_tests.rs` — handler and dispatch integration tests (12 tests):
  - list_caches: empty root, finds caches with/without manifests, deterministic, nonexistent root
  - inspect_cache: valid cache, missing cache, invalid manifest, deterministic, path traversal rejection
  - dispatch: tools/list advertises all 3, list_caches via tools/call, inspect_cache via tools/call

### Test Results

```
tests/determinism.rs        6 passed
tests/handler_tests.rs     12 passed
tests/mcp_error_schema.rs   1 passed
tests/schema_harness.rs     1 passed
────────────────────────────────────
Total                       20 passed
```

---

## Remaining Work

### P2 — Test gaps

- [ ] **Init gate test** — Integration test: send `tools/list` before `initialize`, assert `-32600`
- [ ] **Timeout test** — Unit test: verify timeout fires for slow operations
- [ ] **Large message rejection test** — Send >1MiB line, assert parse error response
- [ ] **Budget edge cases** — 0, negative, very large values
- [ ] **Empty/whitespace query** — Confirm empty queries return all documents at score 0.0
- [ ] **`io_error` vs `cache_invalid`** — Test that permission-denied returns `io_error`, missing manifest returns `cache_invalid`

### P3 — Nice to have

- [ ] **`notifications/cancelled` handling** — Accept and silently drop
- [ ] **Structured logging** — Replace `eprintln!` with `tracing` crate
- [ ] **Graceful shutdown** — Handle SIGTERM/SIGINT
- [ ] **Batch JSON-RPC** — Not required for stdio MCP, but spec-correct

---

## Resolved Spec Issues

All spec ambiguities have been decided and implemented:

| # | Issue | Decision | Specs updated |
|---|---|---|---|
| 1 | `mcp_interface.md` stale names | `context.resolve.md` is canonical; interface spec aligned | `mcp_interface.md` |
| 2 | Provenance field | Not v0; future versions MAY add without affecting selection | `context.resolve.md`, `mcp_interface.md` |
| 3 | Milestone scope | MCP stdio server is v0 | `milestone_zero.md` |
| 4 | `io_error` vs `cache_invalid` | Structural → `cache_invalid`; OS-level → `io_error` | `error_schema.md` |
| 5 | Auth for stdio | Delegated to OS process boundary, out of scope for v0 | `security_model.md` |
| 6 | Empty query handling | Allowed; returns score 0.0 for all docs | `resolve_context.rs` |
| 7 | `list_caches`/`inspect_cache` specs | Full specs written | `mcp_interface.md` |
| 8 | CLI spec gaps | Added `resolve`, argument specs, exit code 1, merged `ingest`/`build` | `cli_spec.md` |
| 9 | Security model gaps | Added input validation and resource limits sections | `security_model.md` |
| 10 | Output contract contradictions | `milestone_zero.md` aligned to normative `context.resolve.md` | `milestone_zero.md` |

---

## File Inventory

```
mcp-context-server/
├── Cargo.toml
├── spec_refs.md
├── progress.md              ← this file
├── src/
│   ├── main.rs              binary entry point
│   ├── lib.rs               module declarations
│   ├── server.rs            stdio JSON-RPC loop
│   ├── config.rs            ServerConfig from env
│   ├── schema.rs            JSON schema validation utility
│   ├── protocol/
│   │   ├── mod.rs           re-exports
│   │   ├── request.rs       JSON-RPC request types
│   │   └── response.rs      JSON-RPC response + MCP error types
│   └── handlers/
│       ├── mod.rs            dispatch routing (3 tools advertised)
│       ├── resolve_context.rs  context.resolve handler
│       ├── inspect_cache.rs    context.inspect_cache handler
│       ├── list_caches.rs      context.list_caches handler
│       └── health.rs           health check stub
└── tests/
    ├── mcp_error_schema.rs   golden schema test (1 test)
    ├── schema_harness.rs     schema validation harness (1 test)
    ├── determinism.rs        determinism regression suite (6 tests)
    └── handler_tests.rs      handler + dispatch tests (12 tests)
```
