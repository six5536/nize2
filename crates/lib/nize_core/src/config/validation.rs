// @awa-component: CFG-ConfigResolver
//
//! Config value validation — runs validators defined on config definitions.

use crate::models::config::ConfigValidator;

/// Validate a value against a list of validators.
/// Returns a list of validation error messages (empty = valid).
pub fn validate_value(value: &str, validators: &[ConfigValidator]) -> Vec<String> {
    let mut errors = Vec::new();

    for validator in validators {
        match validator.validator_type.as_str() {
            "required" => {
                if value.trim().is_empty() {
                    errors.push(
                        validator
                            .message
                            .clone()
                            .unwrap_or_else(|| "Value is required".to_string()),
                    );
                }
            }
            "min" => {
                if let Some(min_val) = validator.value.as_ref().and_then(|v| v.as_f64()) {
                    if let Ok(num) = value.parse::<f64>() {
                        if num < min_val {
                            errors.push(
                                validator
                                    .message
                                    .clone()
                                    .unwrap_or_else(|| format!("Value must be at least {min_val}")),
                            );
                        }
                    }
                    // If value is a string, check length
                    else if (value.len() as f64) < min_val {
                        errors.push(validator.message.clone().unwrap_or_else(|| {
                            format!("Value must be at least {min_val} characters")
                        }));
                    }
                }
            }
            "max" => {
                if let Some(max_val) = validator.value.as_ref().and_then(|v| v.as_f64())
                    && let Ok(num) = value.parse::<f64>()
                    && num > max_val
                {
                    errors.push(
                        validator
                            .message
                            .clone()
                            .unwrap_or_else(|| format!("Value must be at most {max_val}")),
                    );
                }
            }
            "regex" => {
                if let Some(pattern) = validator.value.as_ref().and_then(|v| v.as_str()) {
                    // Use basic string matching; full regex support can be added later
                    if !value.contains(pattern) {
                        errors.push(
                            validator
                                .message
                                .clone()
                                .unwrap_or_else(|| format!("Value must match pattern: {pattern}")),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_validator(
        vtype: &str,
        value: Option<serde_json::Value>,
        message: Option<&str>,
    ) -> ConfigValidator {
        ConfigValidator {
            validator_type: vtype.to_string(),
            value,
            message: message.map(|m| m.to_string()),
        }
    }

    // @awa-test: CFG_P-1
    #[test]
    fn required_rejects_empty() {
        let validators = vec![make_validator("required", None, Some("Required"))];
        let errors = validate_value("", &validators);
        assert_eq!(errors, vec!["Required"]);
    }

    // @awa-test: CFG_P-1
    #[test]
    fn required_accepts_non_empty() {
        let validators = vec![make_validator("required", None, Some("Required"))];
        let errors = validate_value("hello", &validators);
        assert!(errors.is_empty());
    }

    // @awa-test: CFG_P-1
    #[test]
    fn min_rejects_below_threshold() {
        let validators = vec![make_validator("min", Some(json!(0)), Some("Must be >= 0"))];
        let errors = validate_value("-1", &validators);
        assert_eq!(errors, vec!["Must be >= 0"]);
    }

    // @awa-test: CFG_P-1
    #[test]
    fn min_accepts_at_threshold() {
        let validators = vec![make_validator("min", Some(json!(0)), Some("Must be >= 0"))];
        let errors = validate_value("0", &validators);
        assert!(errors.is_empty());
    }

    // @awa-test: CFG_P-1
    #[test]
    fn max_rejects_above_threshold() {
        let validators = vec![make_validator("max", Some(json!(2)), Some("Must be <= 2"))];
        let errors = validate_value("3", &validators);
        assert_eq!(errors, vec!["Must be <= 2"]);
    }

    // @awa-test: CFG_P-1
    #[test]
    fn max_accepts_at_threshold() {
        let validators = vec![make_validator("max", Some(json!(2)), Some("Must be <= 2"))];
        let errors = validate_value("2", &validators);
        assert!(errors.is_empty());
    }

    // @awa-test: CFG_P-1
    #[test]
    fn multiple_validators_accumulate_errors() {
        let validators = vec![
            make_validator("min", Some(json!(0)), Some("Min 0")),
            make_validator("max", Some(json!(2)), Some("Max 2")),
        ];
        let errors = validate_value("-1", &validators);
        assert_eq!(errors, vec!["Min 0"]);

        let errors = validate_value("3", &validators);
        assert_eq!(errors, vec!["Max 2"]);

        let errors = validate_value("1", &validators);
        assert!(errors.is_empty());
    }
}
