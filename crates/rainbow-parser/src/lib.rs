//! Syntax parser, AST and canonical formatter for the Rainbow DSL.
//!
//! Core `decode` / `encode` accept templates with `#{...}` placeholders.
//! Backends expand holes via [`expand_placeholders`] before shipping concrete
//! Rainbow to mobile clients.

mod ast;
mod diagnostics;
mod embedded;
mod expand;
mod json;
mod lexer;
mod parser;
mod printer;
mod registry;
mod semantics;

pub use ast::{
    EmbeddedLanguage, RainbowBlock, RainbowDocument, RainbowLeadingTrivia, RainbowName,
    RainbowNode, RainbowObjectEntry, RainbowParameter, RainbowTaggedBody, RainbowUseDeclaration,
    RainbowValue,
};
pub use diagnostics::{
    RainbowDiagnostic, RainbowDiagnosticSeverity, RainbowParseError, RainbowSourceLocation,
    RainbowSourceRange,
};
pub use embedded::{
    analyze_embedded_values, collect_embedded_regions, format_embedded,
    pretty_embedded_in_document, validate_embedded, EmbeddedRegion,
};
pub use expand::{expand_placeholders, parse_substitution_map};
pub use json::{
    diagnostics_to_json, document_to_json, expand_error_to_json, expand_success_to_json,
    format_error_to_json, format_success_to_json, parse_error_to_json, parse_success_to_json,
    validate_error_to_json, validate_success_to_json,
};
pub use lexer::{lex, RainbowToken, RainbowTokenKind};
pub use parser::parse_tokens;
pub use printer::{format_document, RainbowFormatStyle};
pub use registry::{
    decode_name_registry, parse_name_registry, validate_name_registry,
    validate_name_registry_with_source, NameRegistry,
};
pub use semantics::{
    analyze_concrete_document, analyze_document_name_kinds, is_trigger_node_name,
    is_upper_camel_case_name,
};

pub const RAINBOW_PARSER_VERSION: &str = "0.1.0-beta.1";

#[derive(Clone, Copy, Debug, Default)]
pub struct RainbowParser;

impl RainbowParser {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, source: &str) -> Result<RainbowDocument, RainbowParseError> {
        decode(source)
    }

    pub fn encode(&self, document: &RainbowDocument) -> String {
        encode(document)
    }

    pub fn encode_with_style(
        &self,
        document: &RainbowDocument,
        style: &RainbowFormatStyle,
    ) -> String {
        let mut document = document.clone();
        pretty_embedded_in_document(&mut document);
        format_document(&document, style)
    }
}

pub fn decode(source: &str) -> Result<RainbowDocument, RainbowParseError> {
    let tokens = lex(source)?;
    parse_tokens(&tokens)
}

pub fn encode(document: &RainbowDocument) -> String {
    let mut document = document.clone();
    pretty_embedded_in_document(&mut document);
    format_document(&document, &RainbowFormatStyle::default())
}

pub fn format_source(source: &str) -> Result<String, RainbowParseError> {
    decode(source).map(|document| encode(&document))
}

/// Template-aware validation (placeholders allowed; concrete `@LANG` bodies still checked).
pub fn validate_source(source: &str) -> Result<RainbowDocument, RainbowParseError> {
    let document = decode(source)?;
    let mut diagnostics = analyze_document_name_kinds(&document, source);
    diagnostics.extend(analyze_embedded_values(&document, source));
    if diagnostics.is_empty() {
        Ok(document)
    } else {
        Err(RainbowParseError::new(diagnostics))
    }
}

/// Concrete validation: naming + no `#{...}` leftovers + embedded language bodies.
pub fn validate_concrete_source(source: &str) -> Result<RainbowDocument, RainbowParseError> {
    let document = decode(source)?;
    let mut diagnostics = analyze_document_name_kinds(&document, source);
    diagnostics.extend(analyze_concrete_document(&document, source));
    diagnostics.extend(analyze_embedded_values(&document, source));
    if diagnostics.is_empty() {
        Ok(document)
    } else {
        Err(RainbowParseError::new(diagnostics))
    }
}

/// Expand placeholders then validate as concrete Rainbow (backend ship path).
pub fn expand_and_validate(
    source: &str,
    substitutions: &std::collections::BTreeMap<String, String>,
) -> Result<String, RainbowParseError> {
    let expanded = expand_placeholders(source, substitutions)?;
    validate_concrete_source(&expanded)?;
    Ok(expanded)
}
