use crate::ast::{
    RainbowDocument, RainbowLeadingTrivia, RainbowName, RainbowNode, RainbowObjectEntry,
    RainbowParameter, RainbowTaggedBody, RainbowUseDeclaration, RainbowValue,
};
use crate::semantics::is_trigger_node_name;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RainbowFormatStyle {
    pub indentation: String,
    pub newline: String,
}

impl RainbowFormatStyle {
    pub fn new(indentation: impl Into<String>, newline: impl Into<String>) -> Self {
        Self {
            indentation: indentation.into(),
            newline: newline.into(),
        }
    }
}

impl Default for RainbowFormatStyle {
    fn default() -> Self {
        Self {
            indentation: "  ".to_string(),
            newline: "\n".to_string(),
        }
    }
}

pub fn format_document(document: &RainbowDocument, style: &RainbowFormatStyle) -> String {
    RainbowPrinter { style }.print(document)
}

struct RainbowPrinter<'style> {
    style: &'style RainbowFormatStyle,
}

impl RainbowPrinter<'_> {
    fn print(&self, document: &RainbowDocument) -> String {
        let mut sections: Vec<String> = Vec::new();

        if !document.uses.is_empty() {
            let uses = document
                .uses
                .iter()
                .map(|declaration| self.print_use(declaration))
                .collect::<Vec<_>>()
                .join(&self.style.newline);
            sections.push(uses);
        }

        for node in &document.nodes {
            sections.push(self.print_node(node, 0));
        }

        sections.join(&format!("{n}{n}", n = self.style.newline))
    }

    fn print_use(&self, declaration: &RainbowUseDeclaration) -> String {
        let mut parts = Vec::new();
        self.push_leading(&mut parts, &declaration.leading, 0);
        parts.push(declaration.render());
        parts.join(&self.style.newline)
    }

    fn print_node(&self, node: &RainbowNode, level: usize) -> String {
        let indent = self.indentation(level);
        let mut parts = Vec::new();
        self.push_leading(&mut parts, &node.leading, level);

        let mut header = indent.clone();
        header.push_str(&node.name.render());

        if !node.parameters.is_empty() || should_emit_constructor(node) {
            if should_break_parameters(node) {
                header.push('(');
                parts.push(header);
                let inner = self.indentation(level + 1);
                let last = node.parameters.len() - 1;
                for (index, parameter) in node.parameters.iter().enumerate() {
                    parts.extend(self.print_parameter_lines(parameter, &inner));
                    if index != last {
                        if let Some(last_line) = parts.last_mut() {
                            last_line.push(',');
                        }
                    }
                }
                parts.push(format!("{indent})"));
            } else {
                header.push('(');
                if !node.parameters.is_empty() {
                    header.push_str(
                        &node
                            .parameters
                            .iter()
                            .map(|parameter| {
                                self.print_parameter_inline(parameter, &self.indentation(level + 1))
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                }
                header.push(')');
                parts.push(header);
            }
        } else {
            parts.push(header);
        }

        if let Some(block) = &node.block {
            if let Some(last) = parts.last_mut() {
                last.push_str(" {");
            }
            if block.children.is_empty() {
                parts.push(format!("{indent}}}"));
            } else {
                let children = block
                    .children
                    .iter()
                    .map(|child| self.print_node(child, level + 1))
                    .collect::<Vec<_>>()
                    .join(&format!("{n}{n}", n = self.style.newline));
                parts.push(children);
                parts.push(format!("{indent}}}"));
            }
        }

        self.push_modifiers(&mut parts, node, level);
        parts.join(&self.style.newline)
    }

    fn push_modifiers(&self, parts: &mut Vec<String>, node: &RainbowNode, level: usize) {
        let indent = self.indentation(level);
        if let Some(id) = &node.id {
            parts.push(format!("{indent}.id(\"{}\")", escape_rainbow_string(id)));
        }
        if !node.style.is_empty() {
            parts.push(self.print_style_modifier(&node.style, level));
        }
        if !node.events.is_empty() {
            parts.push(format!("{indent}.events {{"));
            let trigger_indent = self.indentation(level + 1);
            for trigger in &node.events {
                self.push_leading(parts, &trigger.leading, level + 1);
                parts.push(format!("{trigger_indent}{} {{", trigger.name.render()));
                for event in &trigger.events {
                    parts.push(self.print_node(event, level + 2));
                }
                parts.push(format!("{trigger_indent}}}"));
            }
            parts.push(format!("{indent}}}"));
        }
        if !node.slots.is_empty() {
            parts.push(format!("{indent}.slots {{"));
            let slot_indent = self.indentation(level + 1);
            for slot in &node.slots {
                self.push_leading(parts, &slot.leading, level + 1);
                parts.push(format!("{slot_indent}{} {{", slot.name.render()));
                for plugin in &slot.plugins {
                    parts.push(self.print_node(plugin, level + 2));
                }
                parts.push(format!("{slot_indent}}}"));
            }
            parts.push(format!("{indent}}}"));
        }
    }

    fn print_style_modifier(&self, parameters: &[RainbowParameter], level: usize) -> String {
        let indent = self.indentation(level);
        let complex = parameters.len() >= 2 || parameters.iter().any(parameter_needs_break);
        if !complex {
            let inline = parameters
                .iter()
                .map(|parameter| {
                    self.print_parameter_inline(parameter, &self.indentation(level + 1))
                })
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{indent}.style({inline})");
        }

        let mut lines = vec![format!("{indent}.style(")];
        let inner = self.indentation(level + 1);
        let last = parameters.len() - 1;
        for (index, parameter) in parameters.iter().enumerate() {
            let mut entry = self.print_parameter_lines(parameter, &inner);
            if index != last {
                if let Some(last_line) = entry.last_mut() {
                    last_line.push(',');
                }
            }
            lines.extend(entry);
        }
        lines.push(format!("{indent})"));
        lines.join(&self.style.newline)
    }

    fn print_parameter_lines(&self, parameter: &RainbowParameter, indent: &str) -> Vec<String> {
        let mut lines = Vec::new();
        self.push_leading_raw(&mut lines, &parameter.leading, indent);
        let value = self.print_value(&parameter.value, indent);
        let value_lines: Vec<&str> = value.lines().collect();
        if value_lines.len() <= 1 {
            lines.push(format!("{indent}{}: {value}", parameter.name));
        } else {
            lines.push(format!("{indent}{}: {}", parameter.name, value_lines[0]));
            for line in &value_lines[1..] {
                lines.push((*line).to_string());
            }
        }
        lines
    }

    fn push_leading(&self, parts: &mut Vec<String>, trivia: &RainbowLeadingTrivia, level: usize) {
        let indent = self.indentation(level);
        self.push_leading_raw(parts, trivia, &indent);
    }

    fn push_leading_raw(
        &self,
        parts: &mut Vec<String>,
        trivia: &RainbowLeadingTrivia,
        indent: &str,
    ) {
        for comment in &trivia.comments {
            parts.push(format!("{indent}//{comment}"));
        }
    }

    fn indentation(&self, level: usize) -> String {
        self.style.indentation.repeat(level)
    }

    fn print_parameter_inline(
        &self,
        parameter: &RainbowParameter,
        continuation_indent: &str,
    ) -> String {
        format!(
            "{}: {}",
            parameter.name,
            self.print_value(&parameter.value, continuation_indent)
        )
    }

    fn print_value(&self, value: &RainbowValue, continuation_indent: &str) -> String {
        match value {
            RainbowValue::String(value) => format!("\"{}\"", escape_rainbow_string(value)),
            RainbowValue::Int(value) => value.to_string(),
            RainbowValue::Double(value) => print_double(*value),
            RainbowValue::Bool(value) => value.to_string(),
            RainbowValue::Identifier(value) => format!(".{value}"),
            RainbowValue::Placeholder(value) => format!("#{{{value}}}"),
            RainbowValue::Tagged { language, body, .. } => match body {
                RainbowTaggedBody::Text(text) => {
                    self.print_tagged_text(language.as_str(), text, continuation_indent)
                }
                RainbowTaggedBody::Placeholder(name) => {
                    format!("@{}(#{{{name}}})", language.as_str())
                }
            },
            RainbowValue::Array(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(|value| self.print_value(value, continuation_indent))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            RainbowValue::Object(entries) => self.print_object(entries, continuation_indent),
            RainbowValue::Plugins(plugins) => self.print_plugin_list(plugins, continuation_indent),
            RainbowValue::Null => "null".to_string(),
        }
    }

    fn print_plugin_list(&self, plugins: &[RainbowNode], continuation_indent: &str) -> String {
        if plugins.is_empty() {
            return "{}".to_string();
        }
        let level = self.level_of(continuation_indent) + 1;
        let mut lines = vec!["{".to_string()];
        for plugin in plugins {
            lines.push(self.print_node(plugin, level));
        }
        lines.push(format!("{continuation_indent}}}"));
        lines.join(&self.style.newline)
    }

    fn level_of(&self, indent: &str) -> usize {
        if self.style.indentation.is_empty() {
            return 0;
        }
        indent.len() / self.style.indentation.len()
    }

    fn print_object(&self, entries: &[RainbowObjectEntry], continuation_indent: &str) -> String {
        if !object_needs_break(entries) {
            return format!(
                "({})",
                entries
                    .iter()
                    .map(|entry| format!(
                        "{}: {}",
                        print_object_key(&entry.key),
                        self.print_value(&entry.value, continuation_indent)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        let inner = format!("{continuation_indent}{}", self.style.indentation);
        let mut lines = vec!["(".to_string()];
        let last = entries.len().saturating_sub(1);
        for (index, entry) in entries.iter().enumerate() {
            let mut entry_lines = self.print_object_entry_lines(entry, &inner);
            if index != last {
                if let Some(last_line) = entry_lines.last_mut() {
                    last_line.push(',');
                }
            }
            lines.extend(entry_lines);
        }
        lines.push(format!("{continuation_indent})"));
        lines.join(&self.style.newline)
    }

    fn print_object_entry_lines(&self, entry: &RainbowObjectEntry, indent: &str) -> Vec<String> {
        let value = self.print_value(&entry.value, indent);
        let value_lines: Vec<&str> = value.lines().collect();
        let key = print_object_key(&entry.key);
        if value_lines.len() <= 1 {
            vec![format!("{indent}{key}: {value}")]
        } else {
            let mut lines = vec![format!("{indent}{key}: {}", value_lines[0])];
            for line in &value_lines[1..] {
                lines.push((*line).to_string());
            }
            lines
        }
    }

    fn print_tagged_text(&self, language: &str, text: &str, continuation_indent: &str) -> String {
        let text = text.trim_end_matches('\n');
        if !text.contains('\n') {
            return format!("@{language}({text})");
        }

        let mut lines = text.lines();
        let first = lines.next().unwrap_or("");
        let mut output = format!("@{language}({first}");
        for line in lines {
            output.push_str(&self.style.newline);
            output.push_str(continuation_indent);
            output.push_str(line);
        }
        output.push(')');
        output
    }
}

/// Plugins and events are always constructors. Triggers and `#{Name}` holes are not.
fn should_emit_constructor(node: &RainbowNode) -> bool {
    match &node.name {
        RainbowName::Placeholder(_) => false,
        RainbowName::Ident(name) => !is_trigger_node_name(name),
    }
}

/// Break a call when it has two or more parameters, or any value that cannot
/// stay on one line (multiline `@LANG`, or an object that itself breaks).
/// `On*` triggers and placeholder nodes stay compact unless a value breaks.
fn should_break_parameters(node: &RainbowNode) -> bool {
    if node.parameters.is_empty() {
        return false;
    }
    let complex = node.parameters.iter().any(parameter_needs_break);
    if node.name.is_placeholder() || is_trigger_node_name(node.name.as_str()) {
        return complex;
    }
    node.parameters.len() >= 2 || complex
}

fn parameter_needs_break(parameter: &RainbowParameter) -> bool {
    value_needs_break(&parameter.value)
}

fn value_needs_break(value: &RainbowValue) -> bool {
    match value {
        RainbowValue::Tagged {
            body: RainbowTaggedBody::Text(text),
            ..
        } => text.contains('\n'),
        RainbowValue::Object(entries) => object_needs_break(entries),
        RainbowValue::Array(values) => values.iter().any(value_needs_break),
        RainbowValue::Plugins(_) => true,
        _ => false,
    }
}

/// Two or more fields always break. A single field stays compact unless its
/// value breaks, so nested objects open at every level that needs it.
fn object_needs_break(entries: &[RainbowObjectEntry]) -> bool {
    entries.len() >= 2 || entries.iter().any(|entry| value_needs_break(&entry.value))
}

fn print_object_key(key: &str) -> String {
    if is_rainbow_printable_identifier(key) {
        key.to_string()
    } else {
        format!("\"{}\"", escape_rainbow_string(key))
    }
}

fn print_double(value: f64) -> String {
    let mut text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

fn escape_rainbow_string(value: &str) -> String {
    let mut output = String::new();

    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\0' => output.push_str("\\0"),
            _ => output.push(character),
        }
    }

    output
}

fn is_rainbow_printable_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    is_rainbow_identifier_start(first) && characters.all(is_rainbow_identifier_continuation)
}

fn is_rainbow_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_rainbow_identifier_continuation(character: char) -> bool {
    is_rainbow_identifier_start(character) || character.is_ascii_digit()
}
