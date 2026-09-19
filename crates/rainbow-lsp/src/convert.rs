use rainbow_parser::{
    RainbowDiagnostic, RainbowDiagnosticSeverity, RainbowDocument, RainbowNode,
    RainbowSourceLocation, RainbowUseDeclaration,
};
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DocumentSymbol, Position, Range, SymbolKind,
};

pub fn position(location: RainbowSourceLocation) -> Position {
    Position::new(
        location.line.saturating_sub(1) as u32,
        location.column.saturating_sub(1) as u32,
    )
}

pub fn range_from_locations(start: RainbowSourceLocation, end: RainbowSourceLocation) -> Range {
    Range::new(position(start), position(end))
}

pub fn line_character_range(line: u32, start_character: u32, end_character: u32) -> Range {
    Range::new(
        Position::new(line, start_character),
        Position::new(line, end_character),
    )
}

pub fn diagnostic(diag: &RainbowDiagnostic) -> Diagnostic {
    let severity = match diag.severity {
        RainbowDiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
        RainbowDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
    };

    Diagnostic {
        range: range_from_locations(diag.range.start, diag.range.end),
        severity: Some(severity),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            diag.code.clone(),
        )),
        source: Some("rainbow-lsp".to_string()),
        message: diag.message.clone(),
        ..Diagnostic::default()
    }
}

pub fn document_symbols(document: &RainbowDocument) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for (index, declaration) in document.uses.iter().enumerate() {
        symbols.push(use_symbol(declaration, index as u32));
    }

    for (index, node) in document.nodes.iter().enumerate() {
        symbols.push(node_symbol(node, index as u32));
    }

    symbols
}

#[allow(deprecated)]
fn use_symbol(declaration: &RainbowUseDeclaration, line: u32) -> DocumentSymbol {
    let label = declaration.render();
    let range = line_character_range(line, 0, label.len().min(1) as u32);
    DocumentSymbol {
        name: label,
        detail: Some(if declaration.version.is_some() {
            "version pin".to_string()
        } else {
            "template use pin (latest)".to_string()
        }),
        kind: SymbolKind::NAMESPACE,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

#[allow(deprecated)]
fn node_symbol(node: &RainbowNode, line: u32) -> DocumentSymbol {
    let children = node
        .children()
        .iter()
        .enumerate()
        .map(|(index, child)| node_symbol(child, line + 1 + index as u32))
        .collect::<Vec<_>>();

    let rendered = node.name.render();
    let range = line_character_range(line, 0, rendered.len() as u32);
    DocumentSymbol {
        name: rendered,
        detail: if node.parameters.is_empty() {
            None
        } else {
            Some(format!("{} parameter(s)", node.parameters.len()))
        },
        kind: SymbolKind::STRUCT,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

/// Best-effort hover using the decoded document + identifier under the cursor.
pub fn hover_at(source: &str, document: &RainbowDocument, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let identifier = identifier_at(line, position.character as usize)?;

    if identifier == "use" {
        return Some(
            "**use**\n\nConcrete pin: `use PluginOrEvent@MAJOR.MINOR.PATCH` (version required).\n\nTemplate pin: `use #{Name}` — expand map value must be `Name@MAJOR.MINOR.PATCH`."
                .to_string(),
        );
    }

    if let Some(declaration) = document
        .uses
        .iter()
        .find(|declaration| declaration.name.as_str() == identifier)
    {
        return Some(format!(
            "**{}**\n\n{}",
            declaration.render(),
            match &declaration.version {
                Some(version) => format!("Pinned at `{version}`."),
                None => "Template use pin — expand to `Name@MAJOR.MINOR.PATCH`.".to_string(),
            }
        ));
    }

    if let Some(node) = find_node_named(&document.nodes, &identifier) {
        return Some(format!(
            "**{}**\n\nRainbow syntax node.\n\n- parameters: {}\n- child nodes: {}",
            node.name.render(),
            node.parameters.len(),
            node.children().len()
        ));
    }

    Some(format!("**{identifier}**\n\nRainbow identifier."))
}

fn find_node_named<'a>(nodes: &'a [RainbowNode], name: &str) -> Option<&'a RainbowNode> {
    for node in nodes {
        if node.name == name {
            return Some(node);
        }
        if let Some(found) = find_node_named(node.children(), name) {
            return Some(found);
        }
    }
    None
}

fn identifier_at(line: &str, character: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let index = character.min(chars.len().saturating_sub(1));
    if !is_ident_char(chars[index]) {
        return None;
    }

    let mut start = index;
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }

    let mut end = index + 1;
    while end < chars.len() && is_ident_char(chars[end]) {
        end += 1;
    }

    Some(chars[start..end].iter().collect())
}

fn is_ident_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}
