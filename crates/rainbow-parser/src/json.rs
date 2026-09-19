use crate::ast::{
    RainbowDocument, RainbowLeadingTrivia, RainbowName, RainbowNode, RainbowObjectEntry,
    RainbowParameter, RainbowValue,
};
use crate::diagnostics::{
    RainbowDiagnostic, RainbowDiagnosticSeverity, RainbowSourceLocation, RainbowSourceRange,
};

pub fn parse_success_to_json(document: &RainbowDocument) -> String {
    format!(
        "{{\"ok\":true,\"document\":{},\"diagnostics\":[]}}",
        document_to_json(document)
    )
}

pub fn parse_error_to_json(diagnostics: &[RainbowDiagnostic]) -> String {
    format!(
        "{{\"ok\":false,\"document\":null,\"diagnostics\":{}}}",
        diagnostics_to_json(diagnostics)
    )
}

pub fn format_success_to_json(formatted: &str) -> String {
    format!(
        "{{\"ok\":true,\"formatted\":{},\"diagnostics\":[]}}",
        json_string(formatted)
    )
}

pub fn format_error_to_json(diagnostics: &[RainbowDiagnostic]) -> String {
    format!(
        "{{\"ok\":false,\"formatted\":null,\"diagnostics\":{}}}",
        diagnostics_to_json(diagnostics)
    )
}

pub fn validate_success_to_json() -> String {
    "{\"ok\":true,\"diagnostics\":[]}".to_string()
}

pub fn validate_error_to_json(diagnostics: &[RainbowDiagnostic]) -> String {
    format!(
        "{{\"ok\":false,\"diagnostics\":{}}}",
        diagnostics_to_json(diagnostics)
    )
}

pub fn expand_success_to_json(expanded: &str) -> String {
    format!(
        "{{\"ok\":true,\"expanded\":{},\"diagnostics\":[]}}",
        json_string(expanded)
    )
}

pub fn expand_error_to_json(diagnostics: &[RainbowDiagnostic]) -> String {
    format!(
        "{{\"ok\":false,\"expanded\":null,\"diagnostics\":{}}}",
        diagnostics_to_json(diagnostics)
    )
}

pub fn document_to_json(document: &RainbowDocument) -> String {
    format!(
        "{{\"uses\":[{}],\"nodes\":[{}]}}",
        document
            .uses
            .iter()
            .map(use_to_json)
            .collect::<Vec<_>>()
            .join(","),
        document
            .nodes
            .iter()
            .map(node_to_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn use_to_json(declaration: &crate::ast::RainbowUseDeclaration) -> String {
    let version = match &declaration.version {
        Some(version) => json_string(version),
        None => "null".to_string(),
    };
    format!(
        "{{\"name\":{},\"version\":{},\"leading\":{}}}",
        name_to_json(&declaration.name),
        version,
        leading_to_json(&declaration.leading)
    )
}

fn leading_to_json(trivia: &RainbowLeadingTrivia) -> String {
    format!(
        "{{\"comments\":[{}]}}",
        trivia
            .comments
            .iter()
            .map(|comment| json_string(comment))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn name_to_json(name: &RainbowName) -> String {
    match name {
        RainbowName::Ident(value) => {
            format!("{{\"kind\":\"ident\",\"value\":{}}}", json_string(value))
        }
        RainbowName::Placeholder(value) => format!(
            "{{\"kind\":\"placeholder\",\"value\":{}}}",
            json_string(value)
        ),
    }
}

pub fn diagnostics_to_json(diagnostics: &[RainbowDiagnostic]) -> String {
    format!(
        "[{}]",
        diagnostics
            .iter()
            .map(diagnostic_to_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn node_to_json(node: &RainbowNode) -> String {
    let block = node
        .block
        .as_ref()
        .map(|block| {
            format!(
                "{{\"children\":[{}]}}",
                block
                    .children
                    .iter()
                    .map(node_to_json)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .unwrap_or_else(|| "null".to_string());

    format!(
        "{{\"name\":{},\"parameters\":[{}],\"block\":{},\"leading\":{}}}",
        name_to_json(&node.name),
        node.parameters
            .iter()
            .map(parameter_to_json)
            .collect::<Vec<_>>()
            .join(","),
        block,
        leading_to_json(&node.leading)
    )
}

fn parameter_to_json(parameter: &RainbowParameter) -> String {
    format!(
        "{{\"name\":{},\"value\":{},\"leading\":{}}}",
        json_string(&parameter.name),
        value_to_json(&parameter.value),
        leading_to_json(&parameter.leading)
    )
}

fn value_to_json(value: &RainbowValue) -> String {
    match value {
        RainbowValue::String(value) => {
            format!("{{\"kind\":\"string\",\"value\":{}}}", json_string(value))
        }
        RainbowValue::Int(value) => format!("{{\"kind\":\"int\",\"value\":{value}}}"),
        RainbowValue::Double(value) => {
            format!("{{\"kind\":\"double\",\"value\":{}}}", json_number(*value))
        }
        RainbowValue::Bool(value) => format!("{{\"kind\":\"bool\",\"value\":{value}}}"),
        RainbowValue::Identifier(value) => {
            format!(
                "{{\"kind\":\"identifier\",\"value\":{}}}",
                json_string(value)
            )
        }
        RainbowValue::Placeholder(value) => format!(
            "{{\"kind\":\"placeholder\",\"value\":{}}}",
            json_string(value)
        ),
        RainbowValue::Tagged {
            language,
            body,
            body_range,
        } => {
            let body_json = match body {
                crate::ast::RainbowTaggedBody::Text(text) => {
                    format!("{{\"kind\":\"text\",\"value\":{}}}", json_string(text))
                }
                crate::ast::RainbowTaggedBody::Placeholder(name) => format!(
                    "{{\"kind\":\"placeholder\",\"value\":{}}}",
                    json_string(name)
                ),
            };
            format!(
                "{{\"kind\":\"tagged\",\"language\":{},\"body\":{},\"bodyRange\":{}}}",
                json_string(language.as_str()),
                body_json,
                range_to_json(*body_range)
            )
        }
        RainbowValue::Array(values) => format!(
            "{{\"kind\":\"array\",\"values\":[{}]}}",
            values
                .iter()
                .map(value_to_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        RainbowValue::Object(entries) => format!(
            "{{\"kind\":\"object\",\"entries\":[{}]}}",
            entries
                .iter()
                .map(object_entry_to_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        RainbowValue::Null => "{\"kind\":\"null\"}".to_string(),
    }
}

fn object_entry_to_json(entry: &RainbowObjectEntry) -> String {
    format!(
        "{{\"key\":{},\"value\":{}}}",
        json_string(&entry.key),
        value_to_json(&entry.value)
    )
}

fn diagnostic_to_json(diagnostic: &RainbowDiagnostic) -> String {
    format!(
        "{{\"code\":{},\"message\":{},\"severity\":{},\"range\":{}}}",
        json_string(&diagnostic.code),
        json_string(&diagnostic.message),
        json_string(severity_to_str(diagnostic.severity)),
        range_to_json(diagnostic.range)
    )
}

fn range_to_json(range: RainbowSourceRange) -> String {
    format!(
        "{{\"start\":{},\"end\":{}}}",
        location_to_json(range.start),
        location_to_json(range.end)
    )
}

fn location_to_json(location: RainbowSourceLocation) -> String {
    format!(
        "{{\"offset\":{},\"line\":{},\"column\":{}}}",
        location.offset, location.line, location.column
    )
}

fn severity_to_str(severity: RainbowDiagnosticSeverity) -> &'static str {
    severity.as_str()
}

fn json_number(value: f64) -> String {
    if value.is_finite() {
        let mut text = value.to_string();
        if !text.contains('.') && !text.contains('e') && !text.contains('E') {
            text.push_str(".0");
        }
        text
    } else {
        "null".to_string()
    }
}

fn json_string(value: &str) -> String {
    let mut output = String::from("\"");

    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            character if character <= '\u{1f}' => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            _ => output.push(character),
        }
    }

    output.push('"');
    output
}
