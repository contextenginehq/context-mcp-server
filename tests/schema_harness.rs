use mcp_context_server::schema::validate_json;

#[test]
fn json_schema_harness_validates_instance() {
    let schema = r#"{
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "type": "object",
      "required": ["error"],
      "additionalProperties": false,
      "properties": {
        "error": {
          "type": "object",
          "required": ["code", "message"],
          "additionalProperties": false,
          "properties": {
            "code": { "type": "string" },
            "message": { "type": "string", "minLength": 1 }
          }
        }
      }
    }"#;

    let instance = r#"{
      "error": {
        "code": "cache_invalid",
        "message": "Cache exists but is invalid"
      }
    }"#;

    validate_json(schema, instance).expect("schema validation failed");
}
