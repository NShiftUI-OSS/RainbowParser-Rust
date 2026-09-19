use std::collections::{BTreeSet, HashSet};

use crate::diagnostics::{
    RainbowDiagnostic, RainbowParseError, RainbowSourceLocation, RainbowSourceRange,
};

/// Known plugin and event names for tooling (CLI / LSP).
///
/// Plugin and event names share one namespace: the same name must not appear in both lists.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NameRegistry {
    pub plugins: BTreeSet<String>,
    pub events: BTreeSet<String>,
}

impl NameRegistry {
    pub fn new(
        plugins: impl IntoIterator<Item = impl Into<String>>,
        events: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            plugins: plugins.into_iter().map(Into::into).collect(),
            events: events.into_iter().map(Into::into).collect(),
        }
    }

    pub fn conflicting_names(&self) -> Vec<String> {
        self.plugins.intersection(&self.events).cloned().collect()
    }
}

/// Parse a name registry JSON document.
///
/// Expected shape:
/// ```json
/// { "plugins": ["Screen"], "events": ["Navigate"] }
/// ```
pub fn parse_name_registry(source: &str) -> Result<NameRegistry, RainbowParseError> {
    let value: serde_json::Value = serde_json::from_str(source).map_err(|error| {
        RainbowParseError::single(RainbowDiagnostic::error(
            "rainbow.registry.invalidJson",
            format!("Invalid name registry JSON: {error}"),
            range_at_offset(source, error.column().saturating_sub(1)),
        ))
    })?;

    let object = value.as_object().ok_or_else(|| {
        RainbowParseError::single(RainbowDiagnostic::error(
            "rainbow.registry.invalidJson",
            "Name registry JSON must be an object with \"plugins\" and \"events\" arrays.",
            document_range(source),
        ))
    })?;

    let plugins = read_name_array(object, "plugins", source)?;
    let events = read_name_array(object, "events", source)?;

    Ok(NameRegistry::new(plugins, events))
}

/// Validate that no name appears as both a plugin and an event.
pub fn validate_name_registry(registry: &NameRegistry) -> Vec<RainbowDiagnostic> {
    validate_name_registry_with_source(registry, "")
}

/// Like [`validate_name_registry`], but attaches source ranges when `source` is the registry JSON.
pub fn validate_name_registry_with_source(
    registry: &NameRegistry,
    source: &str,
) -> Vec<RainbowDiagnostic> {
    registry
        .conflicting_names()
        .into_iter()
        .map(|name| {
            let range = find_name_range(source, &name).unwrap_or_else(|| document_range(source));
            RainbowDiagnostic::error(
                "rainbow.semantic.nameKindConflict",
                format!("Name \"{name}\" cannot be both a plugin and an event; choose one kind."),
                range,
            )
        })
        .collect()
}

/// Parse and validate a registry JSON document in one step.
pub fn decode_name_registry(source: &str) -> Result<NameRegistry, RainbowParseError> {
    let registry = parse_name_registry(source)?;
    let diagnostics = validate_name_registry_with_source(&registry, source);
    if diagnostics.is_empty() {
        Ok(registry)
    } else {
        Err(RainbowParseError::new(diagnostics))
    }
}

fn read_name_array(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    source: &str,
) -> Result<Vec<String>, RainbowParseError> {
    let Some(value) = object.get(key) else {
        return Ok(Vec::new());
    };

    let array = value.as_array().ok_or_else(|| {
        RainbowParseError::single(RainbowDiagnostic::error(
            "rainbow.registry.invalidJson",
            format!("Name registry \"{key}\" must be an array of strings."),
            find_key_range(source, key).unwrap_or_else(|| document_range(source)),
        ))
    })?;

    let mut names = Vec::with_capacity(array.len());
    let mut seen = HashSet::new();

    for entry in array {
        let Some(name) = entry.as_str() else {
            return Err(RainbowParseError::single(RainbowDiagnostic::error(
                "rainbow.registry.invalidJson",
                format!("Name registry \"{key}\" entries must be strings."),
                find_key_range(source, key).unwrap_or_else(|| document_range(source)),
            )));
        };

        if !seen.insert(name.to_string()) {
            continue;
        }
        names.push(name.to_string());
    }

    Ok(names)
}

fn document_range(source: &str) -> RainbowSourceRange {
    if source.is_empty() {
        return RainbowSourceRange::new(
            RainbowSourceLocation::new(0, 1, 1),
            RainbowSourceLocation::new(0, 1, 1),
        );
    }
    range_at_offset(source, 0)
}

fn range_at_offset(source: &str, offset: usize) -> RainbowSourceRange {
    let offset = offset.min(source.len());
    let start = offset_to_location(source, offset);
    let end = offset_to_location(source, (offset + 1).min(source.len()));
    RainbowSourceRange::new(start, end)
}

fn offset_to_location(source: &str, offset: usize) -> RainbowSourceLocation {
    let mut line = 1usize;
    let mut column = 1usize;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    RainbowSourceLocation::new(offset, line, column)
}

fn find_key_range(source: &str, key: &str) -> Option<RainbowSourceRange> {
    let needle = format!("\"{key}\"");
    let start = source.find(&needle)?;
    Some(RainbowSourceRange::new(
        offset_to_location(source, start),
        offset_to_location(source, start + needle.len()),
    ))
}

fn find_name_range(source: &str, name: &str) -> Option<RainbowSourceRange> {
    if source.is_empty() {
        return None;
    }
    let needle = format!("\"{name}\"");
    let start = source.find(&needle)?;
    Some(RainbowSourceRange::new(
        offset_to_location(source, start),
        offset_to_location(source, start + needle.len()),
    ))
}
