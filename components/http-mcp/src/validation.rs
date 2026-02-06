use rust_mcp_schema::ToolInputSchema;
use serde_json::Value;

pub fn validate_arguments(
    arguments: &serde_json::Map<String, Value>,
    schema: &ToolInputSchema,
) -> Result<(), String> {
    let args_value = Value::Object(arguments.clone());

    let schema_value =
        serde_json::to_value(schema).map_err(|e| format!("Failed to serialize schema: {}", e))?;

    validate_arguments_value(&args_value, &schema_value)
}

fn validate_arguments_value(arguments: &Value, schema: &Value) -> Result<(), String> {
    let args_obj = arguments.as_object().ok_or("Arguments must be an object")?;

    let properties = schema.get("properties").and_then(|p| p.as_object());
    let required = schema.get("required").and_then(|r| r.as_array());

    if let Some(required_fields) = required {
        for req_field in required_fields {
            let field_name = req_field
                .as_str()
                .ok_or("Required field name must be a string")?;

            if !args_obj.contains_key(field_name) {
                return Err(format!("Missing required argument: {}", field_name));
            }
        }
    }

    if let Some(props) = properties {
        for (arg_name, arg_value) in args_obj {
            if let Some(prop_schema) = props.get(arg_name) {
                validate_value(arg_name, arg_value, prop_schema)?;
            }
        }
    }

    Ok(())
}

fn validate_value(field_name: &str, value: &Value, schema: &Value) -> Result<(), String> {
    let expected_type = schema.get("type").and_then(|t| t.as_str());

    match expected_type {
        Some("string") => validate_string(field_name, value, schema)?,
        Some("number") => validate_number(field_name, value, schema)?,
        Some("integer") => validate_integer(field_name, value, schema)?,
        Some("boolean") => validate_boolean(field_name, value)?,
        Some("array") => validate_array(field_name, value, schema)?,
        Some("object") => validate_object(field_name, value, schema)?,
        Some("null") => {
            if !value.is_null() {
                return Err(format!("Argument '{}' must be null", field_name));
            }
        }
        Some(unknown) => {
            return Err(format!(
                "Unknown type '{}' in schema for '{}'",
                unknown, field_name
            ));
        }
        None => {}
    }

    Ok(())
}

fn validate_string(field_name: &str, value: &Value, schema: &Value) -> Result<(), String> {
    let string_val = value
        .as_str()
        .ok_or_else(|| format!("Argument '{}' must be a string", field_name))?;

    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        let valid_values: Vec<&str> = enum_values.iter().filter_map(|v| v.as_str()).collect();

        if !valid_values.contains(&string_val) {
            return Err(format!(
                "Argument '{}' must be one of: {}. Got: '{}'",
                field_name,
                valid_values.join(", "),
                string_val
            ));
        }
    }

    if let Some(min_len) = schema.get("minLength").and_then(|m| m.as_u64()) {
        if (string_val.len() as u64) < min_len {
            return Err(format!(
                "Argument '{}' must be at least {} characters long",
                field_name, min_len
            ));
        }
    }

    if let Some(max_len) = schema.get("maxLength").and_then(|m| m.as_u64()) {
        if (string_val.len() as u64) > max_len {
            return Err(format!(
                "Argument '{}' must be at most {} characters long",
                field_name, max_len
            ));
        }
    }

    Ok(())
}

fn validate_number(field_name: &str, value: &Value, schema: &Value) -> Result<(), String> {
    let num_val = value
        .as_f64()
        .ok_or_else(|| format!("Argument '{}' must be a number", field_name))?;

    if let Some(minimum) = schema.get("minimum").and_then(|m| m.as_f64()) {
        if num_val < minimum {
            return Err(format!("Argument '{}' must be >= {}", field_name, minimum));
        }
    }

    if let Some(maximum) = schema.get("maximum").and_then(|m| m.as_f64()) {
        if num_val > maximum {
            return Err(format!("Argument '{}' must be <= {}", field_name, maximum));
        }
    }

    if let Some(exclusive_min) = schema.get("exclusiveMinimum").and_then(|m| m.as_f64()) {
        if num_val <= exclusive_min {
            return Err(format!(
                "Argument '{}' must be > {}",
                field_name, exclusive_min
            ));
        }
    }

    if let Some(exclusive_max) = schema.get("exclusiveMaximum").and_then(|m| m.as_f64()) {
        if num_val >= exclusive_max {
            return Err(format!(
                "Argument '{}' must be < {}",
                field_name, exclusive_max
            ));
        }
    }

    if let Some(multiple_of) = schema.get("multipleOf").and_then(|m| m.as_f64()) {
        if multiple_of != 0.0 {
            let remainder = num_val % multiple_of;
            if remainder.abs() > f64::EPSILON {
                return Err(format!(
                    "Argument '{}' must be a multiple of {}",
                    field_name, multiple_of
                ));
            }
        }
    }

    Ok(())
}

fn validate_integer(field_name: &str, value: &Value, schema: &Value) -> Result<(), String> {
    let int_val = value
        .as_i64()
        .ok_or_else(|| format!("Argument '{}' must be an integer", field_name))?;

    if let Some(minimum) = schema.get("minimum").and_then(|m| m.as_i64()) {
        if int_val < minimum {
            return Err(format!("Argument '{}' must be >= {}", field_name, minimum));
        }
    }

    if let Some(maximum) = schema.get("maximum").and_then(|m| m.as_i64()) {
        if int_val > maximum {
            return Err(format!("Argument '{}' must be <= {}", field_name, maximum));
        }
    }

    if let Some(multiple_of) = schema.get("multipleOf").and_then(|m| m.as_i64()) {
        if multiple_of != 0 && int_val % multiple_of != 0 {
            return Err(format!(
                "Argument '{}' must be a multiple of {}",
                field_name, multiple_of
            ));
        }
    }

    Ok(())
}

fn validate_boolean(field_name: &str, value: &Value) -> Result<(), String> {
    if !value.is_boolean() {
        return Err(format!("Argument '{}' must be a boolean", field_name));
    }
    Ok(())
}

fn validate_array(field_name: &str, value: &Value, schema: &Value) -> Result<(), String> {
    let array_val = value
        .as_array()
        .ok_or_else(|| format!("Argument '{}' must be an array", field_name))?;

    if let Some(min_items) = schema.get("minItems").and_then(|m| m.as_u64()) {
        if (array_val.len() as u64) < min_items {
            return Err(format!(
                "Argument '{}' must have at least {} items",
                field_name, min_items
            ));
        }
    }

    if let Some(max_items) = schema.get("maxItems").and_then(|m| m.as_u64()) {
        if (array_val.len() as u64) > max_items {
            return Err(format!(
                "Argument '{}' must have at most {} items",
                field_name, max_items
            ));
        }
    }

    if let Some(true) = schema.get("uniqueItems").and_then(|u| u.as_bool()) {
        let mut seen = std::collections::HashSet::new();
        for item in array_val {
            let item_str = serde_json::to_string(item).unwrap_or_default();
            if !seen.insert(item_str) {
                return Err(format!("Argument '{}' must have unique items", field_name));
            }
        }
    }

    if let Some(items_schema) = schema.get("items") {
        for (i, item) in array_val.iter().enumerate() {
            let item_field_name = format!("{}[{}]", field_name, i);
            validate_value(&item_field_name, item, items_schema)?;
        }
    }

    Ok(())
}

fn validate_object(field_name: &str, value: &Value, schema: &Value) -> Result<(), String> {
    let obj_val = value
        .as_object()
        .ok_or_else(|| format!("Argument '{}' must be an object", field_name))?;

    if let Some(min_props) = schema.get("minProperties").and_then(|m| m.as_u64()) {
        if (obj_val.len() as u64) < min_props {
            return Err(format!(
                "Argument '{}' must have at least {} properties",
                field_name, min_props
            ));
        }
    }

    if let Some(max_props) = schema.get("maxProperties").and_then(|m| m.as_u64()) {
        if (obj_val.len() as u64) > max_props {
            return Err(format!(
                "Argument '{}' must have at most {} properties",
                field_name, max_props
            ));
        }
    }

    if let Some(required_fields) = schema.get("required").and_then(|r| r.as_array()) {
        for req_field in required_fields {
            let req_field_name = req_field
                .as_str()
                .ok_or("Required field name must be a string")?;

            if !obj_val.contains_key(req_field_name) {
                return Err(format!(
                    "Object '{}' is missing required property '{}'",
                    field_name, req_field_name
                ));
            }
        }
    }

    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_value) in obj_val {
            if let Some(prop_schema) = properties.get(prop_name) {
                let nested_field_name = format!("{}.{}", field_name, prop_name);
                validate_value(&nested_field_name, prop_value, prop_schema)?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_arguments_required_fields() {
        let schema = json!({
            "type": "object",
            "properties": {
                "location": { "type": "string" },
                "unit": { "type": "string" }
            },
            "required": ["location"]
        });

        // Valid arguments
        let valid_args = json!({
            "location": "Amsterdam",
            "unit": "celsius"
        });
        let valid_args_map = valid_args.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(valid_args_map, &schema_obj).is_ok());

        // Missing required field
        let invalid_args = json!({
            "unit": "celsius"
        });
        let invalid_args_map = invalid_args.as_object().unwrap();
        assert!(validate_arguments(invalid_args_map, &schema_obj).is_err());
    }

    #[test]
    fn test_validate_string_enum() {
        let schema = json!({
            "type": "object",
            "properties": {
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"]
                }
            }
        });

        // Valid enum value
        let valid = json!({ "unit": "celsius" });
        let valid_map = valid.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(valid_map, &schema_obj).is_ok());

        // Invalid enum value
        let invalid = json!({ "unit": "kelvin" });
        let invalid_map = invalid.as_object().unwrap();
        assert!(validate_arguments(invalid_map, &schema_obj).is_err());
    }

    #[test]
    fn test_validate_string_length() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 10
                }
            }
        });

        // Valid length
        let valid = json!({ "name": "Alice" });
        let valid_map = valid.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(valid_map, &schema_obj).is_ok());

        // Too short
        let too_short = json!({ "name": "Al" });
        let too_short_map = too_short.as_object().unwrap();
        assert!(validate_arguments(too_short_map, &schema_obj).is_err());

        // Too long
        let too_long = json!({ "name": "Alexander the Great" });
        let too_long_map = too_long.as_object().unwrap();
        assert!(validate_arguments(too_long_map, &schema_obj).is_err());
    }

    #[test]
    fn test_validate_number_range() {
        let schema = json!({
            "type": "object",
            "properties": {
                "age": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 120
                }
            }
        });

        // Valid range
        let valid = json!({ "age": 25 });
        let valid_map = valid.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(valid_map, &schema_obj).is_ok());

        // Below minimum
        let below = json!({ "age": -1 });
        let below_map = below.as_object().unwrap();
        assert!(validate_arguments(below_map, &schema_obj).is_err());

        // Above maximum
        let above = json!({ "age": 150 });
        let above_map = above.as_object().unwrap();
        assert!(validate_arguments(above_map, &schema_obj).is_err());
    }

    #[test]
    fn test_validate_integer() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" }
            }
        });

        // Valid integer
        let valid = json!({ "count": 42 });
        let valid_map = valid.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(valid_map, &schema_obj).is_ok());

        // Float when integer expected
        let float_val = json!({ "count": 42.5 });
        let float_val_map = float_val.as_object().unwrap();
        assert!(validate_arguments(float_val_map, &schema_obj).is_err());
    }

    #[test]
    fn test_validate_boolean_and_array() {
        let schema = json!({
            "type": "object",
            "properties": {
                "active": { "type": "boolean" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "maxItems": 5
                }
            }
        });

        // Valid boolean
        let valid = json!({ "active": true });
        let valid_map = valid.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(valid_map, &schema_obj).is_ok());

        // String when boolean expected
        let invalid = json!({ "active": "true" });
        let invalid_map = invalid.as_object().unwrap();
        assert!(validate_arguments(invalid_map, &schema_obj).is_err());

        // Valid array
        let valid = json!({ "tags": ["rust", "wasm"] });
        let valid_map = valid.as_object().unwrap();
        assert!(validate_arguments(valid_map, &schema_obj).is_ok());

        // Empty array (below minItems)
        let empty = json!({ "tags": [] });
        let empty_map = empty.as_object().unwrap();
        assert!(validate_arguments(empty_map, &schema_obj).is_err());

        // Too many items
        let too_many = json!({ "tags": ["a", "b", "c", "d", "e", "f"] });
        let too_many_map = too_many.as_object().unwrap();
        assert!(validate_arguments(too_many_map, &schema_obj).is_err());

        // Invalid item type
        let wrong_type = json!({ "tags": ["valid", 123] });
        let wrong_type_map = wrong_type.as_object().unwrap();
        assert!(validate_arguments(wrong_type_map, &schema_obj).is_err());
    }

    #[test]
    fn test_validate_nested_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "age": { "type": "integer" }
                    },
                    "required": ["name"]
                }
            }
        });

        // Valid nested object
        let valid = json!({
            "user": {
                "name": "Alice",
                "age": 30
            }
        });
        let valid_map = valid.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(valid_map, &schema_obj).is_ok());

        // Missing required nested field
        let invalid = json!({
            "user": {
                "age": 30
            }
        });
        let invalid_map = invalid.as_object().unwrap();
        assert!(validate_arguments(invalid_map, &schema_obj).is_err());
    }

    #[test]
    fn test_validate_type_mismatch() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "count": { "type": "number" }
            }
        });

        // Number where string expected
        let wrong_type = json!({
            "name": 123,
            "count": 5
        });
        let wrong_type_map = wrong_type.as_object().unwrap();
        let schema_obj: rust_mcp_schema::ToolInputSchema =
            serde_json::from_value(schema.clone()).unwrap();
        assert!(validate_arguments(wrong_type_map, &schema_obj).is_err());
    }
}
