use jsonschema::validator_for;
use serde_json::Value;

use mcp_context_server::protocol::{McpErrorCode, McpErrorResponse};

#[test]
fn golden_mcp_error_schema_validation() {
    // 1. Build a canonical error response
    let response = McpErrorResponse::new(
        McpErrorCode::CacheInvalid,
        "Cache exists but is invalid",
    );

    let json_str = serde_json::to_string_pretty(&response).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    // 2. Schema (v0) — frozen
    let schema_str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://context.dev/schemas/mcp/error-v0.json",
  "title": "MCP Error Response v0",
  "type": "object",
  "required": ["error"],
  "additionalProperties": false,
  "properties": {
    "error": {
      "type": "object",
      "required": ["code", "message"],
      "additionalProperties": false,
      "properties": {
        "code": {
          "type": "string",
          "enum": [
            "cache_missing",
            "cache_invalid",
            "invalid_query",
            "invalid_budget",
            "io_error",
            "internal_error"
          ]
        },
        "message": {
          "type": "string",
          "minLength": 1
        }
      }
    }
  }
}"#;

    let schema_json: Value = serde_json::from_str(schema_str).unwrap();
    let validator = validator_for(&schema_json).unwrap();

    // 3. Validate against schema
    assert!(validator.is_valid(&json_value), "MCP error JSON must satisfy v0 schema");

    // 4. Golden snapshot (byte-identical, stable)
    let expected = r#"{
  "error": {
    "code": "cache_invalid",
    "message": "Cache exists but is invalid"
  }
}"#;

    assert_eq!(json_str.trim(), expected.trim(), "MCP error JSON snapshot mismatch");
}
