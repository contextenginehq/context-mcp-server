use jsonschema::validator_for;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum SchemaValidationError {
    #[error("Schema parse error: {0}")]
    SchemaParse(#[from] serde_json::Error),
    #[error("Schema compile error: {0}")]
    SchemaCompile(String),
    #[error("Instance validation failed")]
    ValidationFailed,
}

/// Validate a JSON instance against a JSON Schema (draft 2020-12).
/// Returns Ok(()) if valid, Err otherwise.
pub fn validate_json(schema_str: &str, instance_str: &str) -> Result<(), SchemaValidationError> {
    let schema_json: Value = serde_json::from_str(schema_str)?;
    let instance_json: Value = serde_json::from_str(instance_str)?;

    let validator = validator_for(&schema_json)
        .map_err(|e| SchemaValidationError::SchemaCompile(e.to_string()))?;

    if validator.is_valid(&instance_json) {
        Ok(())
    } else {
        Err(SchemaValidationError::ValidationFailed)
    }
}
