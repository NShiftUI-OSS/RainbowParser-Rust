use std::collections::BTreeMap;

use crate::diagnostics::{
    RainbowDiagnostic, RainbowParseError, RainbowSourceLocation, RainbowSourceRange,
};
use crate::lexer::{lex, RainbowTokenKind};

/// Expand every `#{Name}` in `source` using `substitutions`.
///
/// Replacement values are inserted verbatim (Rainbow fragments). The same name
/// is replaced in every occurrence. Missing names produce diagnostics.
///
/// This is the backend integration point for CMS / multi-backend pipelines:
/// template → `expand_placeholders` → concrete Rainbow → validate / ship.
pub fn expand_placeholders(
    source: &str,
    substitutions: &BTreeMap<String, String>,
) -> Result<String, RainbowParseError> {
    let tokens = lex(source)?;
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    let mut diagnostics = Vec::new();

    for token in &tokens {
        match &token.kind {
            RainbowTokenKind::Placeholder(name) => {
                let start = token.range.start.offset;
                let end = token.range.end.offset;
                if start >= cursor {
                    output.push_str(&source[cursor..start]);
                }

                match substitutions.get(name) {
                    Some(replacement) => output.push_str(replacement),
                    None => {
                        diagnostics.push(RainbowDiagnostic::error(
                            "rainbow.expand.missingSubstitution",
                            format!("No substitution provided for placeholder '#{{{name}}}'."),
                            token.range,
                        ));
                        output.push_str(&source[start..end]);
                    }
                }
                cursor = end;
            }
            RainbowTokenKind::Tagged {
                language,
                body: crate::ast::RainbowTaggedBody::Placeholder(name),
                ..
            } => {
                let start = token.range.start.offset;
                let end = token.range.end.offset;
                if start >= cursor {
                    output.push_str(&source[cursor..start]);
                }

                match substitutions.get(name) {
                    Some(replacement) => {
                        output.push_str(&format!("@{}({replacement})", language.as_str()));
                    }
                    None => {
                        diagnostics.push(RainbowDiagnostic::error(
                            "rainbow.expand.missingSubstitution",
                            format!("No substitution provided for placeholder '#{{{name}}}'."),
                            token.range,
                        ));
                        output.push_str(&source[start..end]);
                    }
                }
                cursor = end;
            }
            _ => {}
        }
    }

    if cursor < source.len() {
        output.push_str(&source[cursor..]);
    }

    if diagnostics.is_empty() {
        Ok(output)
    } else {
        Err(RainbowParseError::new(diagnostics))
    }
}

/// Parse a JSON object map `{ "Name": "replacement", ... }` for [`expand_placeholders`].
pub fn parse_substitution_map(json: &str) -> Result<BTreeMap<String, String>, RainbowParseError> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|error| {
        RainbowParseError::single(RainbowDiagnostic::error(
            "rainbow.expand.invalidMap",
            format!("Invalid substitution map JSON: {error}"),
            RainbowSourceRange::new(
                RainbowSourceLocation::new(0, 1, 1),
                RainbowSourceLocation::new(0, 1, 1),
            ),
        ))
    })?;

    let object = value.as_object().ok_or_else(|| {
        RainbowParseError::single(RainbowDiagnostic::error(
            "rainbow.expand.invalidMap",
            "Substitution map must be a JSON object of string → string.",
            RainbowSourceRange::new(
                RainbowSourceLocation::new(0, 1, 1),
                RainbowSourceLocation::new(0, 1, 1),
            ),
        ))
    })?;

    let mut map = BTreeMap::new();
    for (key, entry) in object {
        let Some(text) = entry.as_str() else {
            return Err(RainbowParseError::single(RainbowDiagnostic::error(
                "rainbow.expand.invalidMap",
                format!("Substitution for \"{key}\" must be a string."),
                RainbowSourceRange::new(
                    RainbowSourceLocation::new(0, 1, 1),
                    RainbowSourceLocation::new(0, 1, 1),
                ),
            )));
        };
        map.insert(key.clone(), text.to_string());
    }

    Ok(map)
}
