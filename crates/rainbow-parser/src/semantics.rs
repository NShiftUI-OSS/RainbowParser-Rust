use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{RainbowDocument, RainbowName, RainbowNode};
use crate::diagnostics::{RainbowDiagnostic, RainbowSourceLocation, RainbowSourceRange};

/// Structural context while walking a Rainbow document for kind positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KindContext {
    /// Document roots and plugin `{}` bodies — non-`On*` children are plugins.
    PluginBody,
    /// Inside an `On*` trigger block — non-`On*` children are events.
    EventBody,
}

/// DSL trigger wrapper: `On` + PascalCase continuation (`OnTap`, `OnSuccess`, …).
pub fn is_trigger_node_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('O'))
        && matches!(chars.next(), Some('n'))
        && matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_alphanumeric())
}

/// Plugin / event / trigger names: UpperCamelCase ASCII letters only (parity with
/// `@NShiftPlugin` / `@NShiftEvent` macros).
pub fn is_upper_camel_case_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => {
            chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_lowercase())
        }
        _ => false,
    }
}

/// Validate document naming rules that do not require a DI container:
/// - concrete plugin / event / trigger / `use` names must be UpperCamelCase
/// - no concrete name used both as a plugin and as an event
/// - placeholders skip casing checks (expanded later by backends)
pub fn analyze_document_name_kinds(
    document: &RainbowDocument,
    source: &str,
) -> Vec<RainbowDiagnostic> {
    let mut plugin_uses: BTreeMap<String, Vec<RainbowSourceRange>> = BTreeMap::new();
    let mut event_uses: BTreeMap<String, Vec<RainbowSourceRange>> = BTreeMap::new();
    let mut name_occurrences: BTreeMap<String, usize> = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for declaration in &document.uses {
        match &declaration.name {
            RainbowName::Placeholder(name) => {
                bump_name_occurrence(&mut name_occurrences, &format!("#{{{name}}}"));
            }
            RainbowName::Ident(name) => {
                let skip = bump_name_occurrence(&mut name_occurrences, name);
                let range = next_identifier_range(source, name, skip)
                    .unwrap_or_else(|| document_range(source));
                push_casing_diagnostic(&mut diagnostics, name, "use pin", range);
            }
        }
    }

    for node in &document.nodes {
        walk_node(
            node,
            KindContext::PluginBody,
            source,
            &mut plugin_uses,
            &mut event_uses,
            &mut name_occurrences,
            &mut diagnostics,
        );
    }

    let plugin_names: BTreeSet<_> = plugin_uses.keys().cloned().collect();
    let event_names: BTreeSet<_> = event_uses.keys().cloned().collect();

    for name in plugin_names.intersection(&event_names) {
        let range = event_uses
            .get(name)
            .and_then(|ranges| ranges.first())
            .or_else(|| plugin_uses.get(name).and_then(|ranges| ranges.first()))
            .copied()
            .unwrap_or_else(|| document_range(source));

        diagnostics.push(RainbowDiagnostic::error(
            "rainbow.semantic.nameKindConflict",
            format!(
                "Name \"{name}\" is used as both a plugin and an event; \
plugin and event names must be unique across kinds."
            ),
            range,
        ));
    }

    diagnostics
}

/// Reject remaining `#{...}` holes — use after backend expand for concrete payloads.
pub fn analyze_concrete_document(
    document: &RainbowDocument,
    source: &str,
) -> Vec<RainbowDiagnostic> {
    if !document.contains_placeholders() {
        return Vec::new();
    }

    // Point at the first placeholder token in source when possible.
    let range = source
        .find("#{")
        .map(|offset| {
            let end = source[offset..]
                .find('}')
                .map(|relative| offset + relative + 1)
                .unwrap_or(offset + 2);
            RainbowSourceRange::new(
                offset_to_location(source, offset),
                offset_to_location(source, end),
            )
        })
        .unwrap_or_else(|| document_range(source));

    vec![RainbowDiagnostic::error(
        "rainbow.semantic.unexpandedPlaceholder",
        "Concrete Rainbow cannot contain '#{...}' placeholders; expand them before shipping.",
        range,
    )]
}

fn walk_node(
    node: &RainbowNode,
    context: KindContext,
    source: &str,
    plugin_uses: &mut BTreeMap<String, Vec<RainbowSourceRange>>,
    event_uses: &mut BTreeMap<String, Vec<RainbowSourceRange>>,
    name_occurrences: &mut BTreeMap<String, usize>,
    diagnostics: &mut Vec<RainbowDiagnostic>,
) {
    let children = node.children();

    match &node.name {
        RainbowName::Placeholder(name) => {
            let rendered = format!("#{{{name}}}");
            let skip = bump_name_occurrence(name_occurrences, &rendered);
            let _range = next_literal_range(source, &rendered, skip)
                .unwrap_or_else(|| document_range(source));
            // Standalone `#{Name}` may expand to a full `use …` line or a node.
            for child in children {
                walk_node(
                    child,
                    context,
                    source,
                    plugin_uses,
                    event_uses,
                    name_occurrences,
                    diagnostics,
                );
            }
            return;
        }
        RainbowName::Ident(name) if is_trigger_node_name(name) => {
            let skip = bump_name_occurrence(name_occurrences, name);
            let range =
                next_identifier_range(source, name, skip).unwrap_or_else(|| document_range(source));
            push_casing_diagnostic(diagnostics, name, "trigger", range);

            for child in children {
                walk_node(
                    child,
                    KindContext::EventBody,
                    source,
                    plugin_uses,
                    event_uses,
                    name_occurrences,
                    diagnostics,
                );
            }
            return;
        }
        RainbowName::Ident(_) => {}
    }

    match context {
        KindContext::PluginBody => {
            if let RainbowName::Ident(name) = &node.name {
                let range = record_use(plugin_uses, name, source, name_occurrences);
                push_casing_diagnostic(diagnostics, name, "plugin", range);
            }
            for child in children {
                walk_node(
                    child,
                    KindContext::PluginBody,
                    source,
                    plugin_uses,
                    event_uses,
                    name_occurrences,
                    diagnostics,
                );
            }
        }
        KindContext::EventBody => {
            if let RainbowName::Ident(name) = &node.name {
                let range = record_use(event_uses, name, source, name_occurrences);
                push_casing_diagnostic(diagnostics, name, "event", range);
            }
            for child in children {
                walk_node(
                    child,
                    KindContext::EventBody,
                    source,
                    plugin_uses,
                    event_uses,
                    name_occurrences,
                    diagnostics,
                );
            }
        }
    }
}

fn push_casing_diagnostic(
    diagnostics: &mut Vec<RainbowDiagnostic>,
    name: &str,
    kind: &str,
    range: RainbowSourceRange,
) {
    if is_upper_camel_case_name(name) {
        return;
    }

    diagnostics.push(RainbowDiagnostic::error(
        "rainbow.semantic.invalidNodeName",
        format!(
            "{kind} name \"{name}\" must be UpperCamelCase ASCII letters \
(e.g. Screen, OnTap, SendHTTPRequest)."
        ),
        range,
    ));
}

fn record_use(
    uses: &mut BTreeMap<String, Vec<RainbowSourceRange>>,
    name: &str,
    source: &str,
    name_occurrences: &mut BTreeMap<String, usize>,
) -> RainbowSourceRange {
    let skip = bump_name_occurrence(name_occurrences, name);
    let range = next_identifier_range(source, name, skip).unwrap_or_else(|| document_range(source));
    uses.entry(name.to_string()).or_default().push(range);
    range
}

fn bump_name_occurrence(name_occurrences: &mut BTreeMap<String, usize>, name: &str) -> usize {
    let entry = name_occurrences.entry(name.to_string()).or_insert(0);
    let skip = *entry;
    *entry += 1;
    skip
}

fn next_identifier_range(source: &str, name: &str, skip: usize) -> Option<RainbowSourceRange> {
    let mut seen = 0usize;
    let bytes = source.as_bytes();
    let name_bytes = name.as_bytes();
    let mut index = 0usize;

    while index + name_bytes.len() <= bytes.len() {
        if &bytes[index..index + name_bytes.len()] == name_bytes
            && is_identifier_boundary_before(bytes, index)
            && is_identifier_boundary_after(bytes, index + name_bytes.len())
        {
            if seen == skip {
                let start = offset_to_location(source, index);
                let end = offset_to_location(source, index + name_bytes.len());
                return Some(RainbowSourceRange::new(start, end));
            }
            seen += 1;
            index += name_bytes.len();
            continue;
        }
        index += 1;
    }

    None
}

fn next_literal_range(source: &str, literal: &str, skip: usize) -> Option<RainbowSourceRange> {
    let mut seen = 0usize;
    let mut search_from = 0usize;
    while let Some(relative) = source[search_from..].find(literal) {
        let index = search_from + relative;
        if seen == skip {
            return Some(RainbowSourceRange::new(
                offset_to_location(source, index),
                offset_to_location(source, index + literal.len()),
            ));
        }
        seen += 1;
        search_from = index + literal.len();
    }
    None
}

fn is_identifier_boundary_before(bytes: &[u8], index: usize) -> bool {
    index == 0 || !is_identifier_char(bytes[index - 1])
}

fn is_identifier_boundary_after(bytes: &[u8], index: usize) -> bool {
    bytes
        .get(index)
        .is_none_or(|byte| !is_identifier_char(*byte))
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn document_range(source: &str) -> RainbowSourceRange {
    RainbowSourceRange::new(
        RainbowSourceLocation::new(0, 1, 1),
        offset_to_location(source, source.len().min(1)),
    )
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
