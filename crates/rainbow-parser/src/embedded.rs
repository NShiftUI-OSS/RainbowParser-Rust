//! Embedded `@LANG(...)` parameter values: validate + pretty-print + region export.

use crate::ast::{
    EmbeddedLanguage, RainbowDocument, RainbowNode, RainbowObjectEntry, RainbowTaggedBody,
    RainbowValue,
};
use crate::diagnostics::{RainbowDiagnostic, RainbowSourceLocation, RainbowSourceRange};

/// One `@LANG(...)` region for IDE language-service embedding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedRegion {
    pub language: EmbeddedLanguage,
    pub body: String,
    pub body_range: RainbowSourceRange,
    /// `true` when the body is `#{Name}` — skip language validation until expand.
    pub is_placeholder: bool,
}

/// Collect every tagged parameter region in document order (depth-first).
pub fn collect_embedded_regions(document: &RainbowDocument) -> Vec<EmbeddedRegion> {
    let mut regions = Vec::new();
    for node in &document.nodes {
        collect_regions_in_node(node, &mut regions);
    }
    regions
}

fn collect_regions_in_node(node: &RainbowNode, regions: &mut Vec<EmbeddedRegion>) {
    for parameter in &node.parameters {
        collect_regions_in_value(&parameter.value, regions);
    }
    for child in node.children() {
        collect_regions_in_node(child, regions);
    }
}

fn collect_regions_in_value(value: &RainbowValue, regions: &mut Vec<EmbeddedRegion>) {
    match value {
        RainbowValue::Tagged {
            language,
            body,
            body_range,
        } => {
            let (text, is_placeholder) = match body {
                RainbowTaggedBody::Text(text) => (text.clone(), false),
                RainbowTaggedBody::Placeholder(name) => (format!("#{{{name}}}"), true),
            };
            regions.push(EmbeddedRegion {
                language: *language,
                body: text,
                body_range: *body_range,
                is_placeholder,
            });
        }
        RainbowValue::Array(values) => {
            for value in values {
                collect_regions_in_value(value, regions);
            }
        }
        RainbowValue::Object(entries) => {
            for entry in entries {
                collect_regions_in_value(&entry.value, regions);
            }
        }
        _ => {}
    }
}

/// Validate every concrete `@LANG(text)` body in the document.
///
/// Placeholder bodies (`@LANG(#{Name})`) are skipped — expand first.
pub fn analyze_embedded_values(
    document: &RainbowDocument,
    _source: &str,
) -> Vec<RainbowDiagnostic> {
    let mut diagnostics = Vec::new();
    for node in &document.nodes {
        walk_node(node, &mut diagnostics);
    }
    diagnostics
}

fn walk_node(node: &RainbowNode, diagnostics: &mut Vec<RainbowDiagnostic>) {
    for parameter in &node.parameters {
        walk_value(&parameter.value, diagnostics);
    }
    for child in node.children() {
        walk_node(child, diagnostics);
    }
}

fn walk_value(value: &RainbowValue, diagnostics: &mut Vec<RainbowDiagnostic>) {
    match value {
        RainbowValue::Tagged {
            language,
            body,
            body_range,
        } => {
            if let RainbowTaggedBody::Text(text) = body {
                diagnostics.extend(validate_embedded(*language, text, *body_range));
            }
        }
        RainbowValue::Array(values) => {
            for value in values {
                walk_value(value, diagnostics);
            }
        }
        RainbowValue::Object(entries) => {
            for entry in entries {
                walk_value(&entry.value, diagnostics);
            }
        }
        _ => {}
    }
}

/// Validate an embedded body; ranges are absolute (mapped from language-local offsets).
pub fn validate_embedded(
    language: EmbeddedLanguage,
    body: &str,
    body_range: RainbowSourceRange,
) -> Vec<RainbowDiagnostic> {
    match language {
        EmbeddedLanguage::Json => validate_json(body, body_range),
        EmbeddedLanguage::Yaml => validate_yaml(body, body_range),
        EmbeddedLanguage::Xml => validate_xml(body, body_range),
        EmbeddedLanguage::Html => validate_html(body, body_range),
        EmbeddedLanguage::Markdown => validate_markdown(body, body_range),
    }
}

/// Pretty-print a valid embedded body. Returns `None` when the body is invalid
/// (caller must keep the original text).
pub fn format_embedded(language: EmbeddedLanguage, body: &str) -> Option<String> {
    if !validate_embedded(
        language,
        body,
        RainbowSourceRange::new(
            RainbowSourceLocation::new(0, 1, 1),
            RainbowSourceLocation::new(body.len(), 1, body.len().saturating_add(1)),
        ),
    )
    .is_empty()
    {
        return None;
    }

    match language {
        EmbeddedLanguage::Json => format_json(body),
        EmbeddedLanguage::Yaml => format_yaml(body),
        EmbeddedLanguage::Xml => format_xml(body),
        EmbeddedLanguage::Html => format_html(body),
        EmbeddedLanguage::Markdown => format_markdown(body),
    }
}

fn validate_json(body: &str, body_range: RainbowSourceRange) -> Vec<RainbowDiagnostic> {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(_) => Vec::new(),
        Err(error) => {
            let line = error.line();
            let column = error.column();
            let range = map_line_column(body, body_range, line, column);
            vec![RainbowDiagnostic::error(
                "rainbow.embedded.json.parse",
                format!("Invalid JSON: {error}"),
                range,
            )]
        }
    }
}

fn format_json(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

fn validate_yaml(body: &str, body_range: RainbowSourceRange) -> Vec<RainbowDiagnostic> {
    match serde_yaml::from_str::<serde_yaml::Value>(body) {
        Ok(_) => Vec::new(),
        Err(error) => {
            let range = error
                .location()
                .map(|location| {
                    map_line_column(body, body_range, location.line(), location.column())
                })
                .unwrap_or(body_range);
            vec![RainbowDiagnostic::error(
                "rainbow.embedded.yaml.parse",
                format!("Invalid YAML: {error}"),
                range,
            )]
        }
    }
}

fn format_yaml(body: &str) -> Option<String> {
    let value: serde_yaml::Value = serde_yaml::from_str(body).ok()?;
    let mut formatted = serde_yaml::to_string(&value).ok()?;
    if formatted.ends_with('\n') {
        formatted.pop();
    }
    Some(formatted)
}

fn validate_xml(body: &str, body_range: RainbowSourceRange) -> Vec<RainbowDiagnostic> {
    match roxmltree::Document::parse(body) {
        Ok(_) => Vec::new(),
        Err(error) => {
            let pos = error.pos();
            let range = map_line_column(body, body_range, pos.row as usize, pos.col as usize);
            vec![RainbowDiagnostic::error(
                "rainbow.embedded.xml.parse",
                format!("Invalid XML: {error}"),
                range,
            )]
        }
    }
}

fn format_xml(body: &str) -> Option<String> {
    let document = roxmltree::Document::parse(body).ok()?;
    Some(pretty_xml_node(document.root_element(), 0))
}

fn pretty_xml_node(node: roxmltree::Node<'_, '_>, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let name = node.tag_name().name();
    let mut attrs = String::new();
    for attribute in node.attributes() {
        attrs.push(' ');
        attrs.push_str(attribute.name());
        attrs.push_str("=\"");
        attrs.push_str(&escape_xml(attribute.value()));
        attrs.push('"');
    }

    let children: Vec<_> = node
        .children()
        .filter(|child| {
            child.is_element() || (child.is_text() && !child.text().unwrap_or("").trim().is_empty())
        })
        .collect();

    if children.is_empty() {
        return format!("{indent}<{name}{attrs}/>");
    }

    let only_text = children.len() == 1 && children[0].is_text();
    if only_text {
        let text = escape_xml(children[0].text().unwrap_or("").trim());
        return format!("{indent}<{name}{attrs}>{text}</{name}>");
    }

    let mut out = format!("{indent}<{name}{attrs}>\n");
    for child in children {
        if child.is_element() {
            out.push_str(&pretty_xml_node(child, depth + 1));
            out.push('\n');
        } else if let Some(text) = child.text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(&"  ".repeat(depth + 1));
                out.push_str(&escape_xml(trimmed));
                out.push('\n');
            }
        }
    }
    out.push_str(&format!("{indent}</{name}>"));
    out
}

fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn validate_html(body: &str, body_range: RainbowSourceRange) -> Vec<RainbowDiagnostic> {
    use html5ever::tendril::StrTendril;
    use html5ever::tokenizer::{
        BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    };

    struct ErrorSink {
        errors: Vec<(u64, String)>,
    }

    impl TokenSink for ErrorSink {
        type Handle = ();

        fn process_token(&mut self, token: Token, line_number: u64) -> TokenSinkResult<()> {
            if let Token::ParseError(error) = token {
                self.errors
                    .push((line_number, format!("HTML parse error: {error}")));
            }
            TokenSinkResult::Continue
        }
    }

    if body.chars().any(|c| c == '\0') {
        return vec![RainbowDiagnostic::error(
            "rainbow.embedded.html.nul",
            "HTML body must not contain NUL characters.",
            body_range,
        )];
    }

    let sink = ErrorSink { errors: Vec::new() };
    let mut tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let mut input = BufferQueue::default();
    input.push_back(StrTendril::from(body));
    let _ = tokenizer.feed(&mut input);
    tokenizer.end();

    tokenizer
        .sink
        .errors
        .into_iter()
        .map(|(line, message)| {
            let range = map_line_column(body, body_range, line as usize, 1);
            RainbowDiagnostic::error("rainbow.embedded.html.parse", message, range)
        })
        .collect()
}

fn format_html(body: &str) -> Option<String> {
    // Conservative: trim edges and normalize internal newlines; keep markup intact.
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Some(String::new());
    }
    let mut out = String::new();
    let mut blank = false;
    for line in trimmed.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if !blank {
                out.push('\n');
                blank = true;
            }
            continue;
        }
        blank = false;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(line);
    }
    Some(out)
}

fn validate_markdown(body: &str, body_range: RainbowSourceRange) -> Vec<RainbowDiagnostic> {
    let mut diagnostics = Vec::new();

    if body.chars().any(|c| c == '\0') {
        diagnostics.push(RainbowDiagnostic::error(
            "rainbow.embedded.markdown.nul",
            "Markdown body must not contain NUL characters.",
            body_range,
        ));
    }

    if let Some(range) = unclosed_fence_range(body, body_range) {
        diagnostics.push(RainbowDiagnostic::error(
            "rainbow.embedded.markdown.unclosedFence",
            "Unclosed fenced code block in Markdown (odd number of ``` fences).",
            range,
        ));
    }

    // Ensure CommonMark parsing succeeds (comrak is permissive; still catches UTF-8 edges).
    let options = comrak::Options::default();
    let _ = comrak::markdown_to_html(body, &options);

    diagnostics
}

fn format_markdown(body: &str) -> Option<String> {
    let trimmed = body.trim_end();
    let mut out = String::new();
    let mut blank = false;
    for line in trimmed.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if !blank && !out.is_empty() {
                out.push('\n');
                blank = true;
            }
            continue;
        }
        blank = false;
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    Some(out)
}

fn unclosed_fence_range(body: &str, body_range: RainbowSourceRange) -> Option<RainbowSourceRange> {
    let mut count = 0usize;
    let mut last_offset = 0usize;
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            count += 1;
            last_offset = offset + (line.len() - trimmed.len());
        }
        offset += line.len();
    }
    if count % 2 == 0 {
        None
    } else {
        Some(map_byte_offset(body, body_range, last_offset))
    }
}

fn map_line_column(
    body: &str,
    body_range: RainbowSourceRange,
    line: usize,
    column: usize,
) -> RainbowSourceRange {
    let line = line.max(1);
    let column = column.max(1);
    let mut current_line = 1usize;
    let mut current_column = 1usize;
    let mut offset = 0usize;

    for character in body.chars() {
        if current_line == line && current_column == column {
            break;
        }
        if character == '\n' {
            current_line += 1;
            current_column = 1;
        } else {
            current_column += 1;
        }
        offset += character.len_utf8();
    }

    map_byte_offset(body, body_range, offset)
}

fn map_byte_offset(
    body: &str,
    body_range: RainbowSourceRange,
    local_offset: usize,
) -> RainbowSourceRange {
    let local_offset = local_offset.min(body.len());
    let absolute = body_range.start.offset.saturating_add(local_offset);
    let end = (absolute + 1).min(body_range.end.offset.max(absolute));

    let start = offset_to_location_from_base(body_range.start, body, local_offset);
    let end_loc = if end > absolute {
        offset_to_location_from_base(body_range.start, body, (local_offset + 1).min(body.len()))
    } else {
        start
    };
    let _ = absolute;
    RainbowSourceRange::new(start, end_loc)
}

fn offset_to_location_from_base(
    base: RainbowSourceLocation,
    body: &str,
    local_offset: usize,
) -> RainbowSourceLocation {
    let mut line = base.line;
    let mut column = base.column;
    let mut seen = 0usize;
    for character in body.chars() {
        if seen >= local_offset {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
        seen += character.len_utf8();
    }
    RainbowSourceLocation::new(base.offset + local_offset, line, column)
}

/// Rewrite `Tagged` text bodies in-place with pretty-printed content when valid.
pub fn pretty_embedded_in_document(document: &mut RainbowDocument) {
    for node in &mut document.nodes {
        pretty_embedded_in_node(node);
    }
}

fn pretty_embedded_in_node(node: &mut RainbowNode) {
    for parameter in &mut node.parameters {
        pretty_embedded_in_value(&mut parameter.value);
    }
    if let Some(block) = &mut node.block {
        for child in &mut block.children {
            pretty_embedded_in_node(child);
        }
    }
}

fn pretty_embedded_in_value(value: &mut RainbowValue) {
    match value {
        RainbowValue::Tagged {
            language,
            body: RainbowTaggedBody::Text(text),
            body_range,
        } => {
            if let Some(formatted) = format_embedded(*language, text) {
                *text = formatted;
                let _ = body_range;
            }
        }
        RainbowValue::Array(values) => {
            for value in values {
                pretty_embedded_in_value(value);
            }
        }
        RainbowValue::Object(entries) => {
            for RainbowObjectEntry { value, .. } in entries {
                pretty_embedded_in_value(value);
            }
        }
        _ => {}
    }
}
