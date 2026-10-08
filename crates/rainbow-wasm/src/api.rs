//! JSON envelopes shared with `rainbow <command> --json`.
//!
//! Callers in Node and React always receive a string. Syntax and semantic
//! failures set `"ok": false` instead of throwing.

use rainbow_parser::{
    decode, decode_name_registry, expand_and_validate, expand_error_to_json,
    expand_placeholders, expand_success_to_json, format_error_to_json, format_source,
    format_success_to_json, parse_error_to_json, parse_substitution_map, parse_success_to_json,
    validate_concrete_source, validate_error_to_json, validate_source, validate_success_to_json,
    RAINBOW_PARSER_VERSION,
};

pub fn version() -> String {
    RAINBOW_PARSER_VERSION.to_string()
}

pub fn parse(source: &str) -> String {
    match decode(source) {
        Ok(document) => parse_success_to_json(&document),
        Err(error) => parse_error_to_json(&error.diagnostics),
    }
}

pub fn format(source: &str) -> String {
    match format_source(source) {
        Ok(formatted) => format_success_to_json(&formatted),
        Err(error) => format_error_to_json(&error.diagnostics),
    }
}

/// `concrete` matches `rainbow validate --concrete` (reject leftover `#{Name}`).
pub fn validate(source: &str, concrete: bool) -> String {
    let result = if concrete {
        validate_concrete_source(source)
    } else {
        validate_source(source)
    };
    match result {
        Ok(_) => validate_success_to_json(),
        Err(error) => validate_error_to_json(&error.diagnostics),
    }
}

/// `map_json` is a JSON object of placeholder name → Rainbow fragment.
/// `validate` matches `rainbow expand --validate`.
pub fn expand(source: &str, map_json: &str, validate: bool) -> String {
    let substitutions = match parse_substitution_map(map_json) {
        Ok(substitutions) => substitutions,
        Err(error) => return expand_error_to_json(&error.diagnostics),
    };
    let result = if validate {
        expand_and_validate(source, &substitutions)
    } else {
        expand_placeholders(source, &substitutions)
    };
    match result {
        Ok(expanded) => expand_success_to_json(&expanded),
        Err(error) => expand_error_to_json(&error.diagnostics),
    }
}

pub fn validate_registry(source: &str) -> String {
    match decode_name_registry(source) {
        Ok(_) => validate_success_to_json(),
        Err(error) => validate_error_to_json(&error.diagnostics),
    }
}
