use crate::ast::{
    RainbowDocument, RainbowLeadingTrivia, RainbowNode, RainbowObjectEntry, RainbowParameter,
    RainbowTaggedBody, RainbowUseDeclaration, RainbowValue,
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

        if !node.parameters.is_empty() {
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
                header.push_str(
                    &node
                        .parameters
                        .iter()
                        .map(|parameter| {
                            print_parameter_inline(parameter, &self.indentation(level + 1))
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                header.push(')');
                parts.push(header);
            }
        } else {
            parts.push(header);
        }

        let Some(block) = &node.block else {
            return parts.join(&self.style.newline);
        };

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

        parts.join(&self.style.newline)
    }

    fn print_parameter_lines(&self, parameter: &RainbowParameter, indent: &str) -> Vec<String> {
        let mut lines = Vec::new();
        self.push_leading_raw(&mut lines, &parameter.leading, indent);
        let value = print_value(&parameter.value, indent);
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
}

/// Break when there are two or more parameters, or any multiline `@LANG` body.
/// `On*` triggers stay compact even if they somehow carry parameters.
fn should_break_parameters(node: &RainbowNode) -> bool {
    if node.name.is_placeholder() || is_trigger_node_name(node.name.as_str()) {
        return false;
    }
    node.parameters.len() >= 2 || node.parameters.iter().any(parameter_needs_break)
}

fn parameter_needs_break(parameter: &RainbowParameter) -> bool {
    match &parameter.value {
        RainbowValue::Tagged {
            body: RainbowTaggedBody::Text(text),
            ..
        } => text.contains('\n'),
        _ => false,
    }
}

fn print_parameter_inline(parameter: &RainbowParameter, continuation_indent: &str) -> String {
    format!(
        "{}: {}",
        parameter.name,
        print_value(&parameter.value, continuation_indent)
    )
}

fn print_value(value: &RainbowValue, continuation_indent: &str) -> String {
    match value {
        RainbowValue::String(value) => format!("\"{}\"", escape_rainbow_string(value)),
        RainbowValue::Int(value) => value.to_string(),
        RainbowValue::Double(value) => print_double(*value),
        RainbowValue::Bool(value) => value.to_string(),
        RainbowValue::Identifier(value) => format!(".{value}"),
        RainbowValue::Placeholder(value) => format!("#{{{value}}}"),
        RainbowValue::Tagged { language, body, .. } => match body {
            RainbowTaggedBody::Text(text) => {
                print_tagged_text(language.as_str(), text, continuation_indent)
            }
            RainbowTaggedBody::Placeholder(name) => {
                format!("@{}(#{{{name}}})", language.as_str())
            }
        },
        RainbowValue::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|value| print_value(value, continuation_indent))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        RainbowValue::Object(entries) => format!(
            "({})",
            entries
                .iter()
                .map(|entry| print_object_entry(entry, continuation_indent))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        RainbowValue::Null => "null".to_string(),
    }
}

fn print_tagged_text(language: &str, text: &str, continuation_indent: &str) -> String {
    let text = text.trim_end_matches('\n');
    if !text.contains('\n') {
        return format!("@{language}({text})");
    }

    let mut lines = text.lines();
    let first = lines.next().unwrap_or("");
    let mut output = format!("@{language}({first}");
    for line in lines {
        output.push('\n');
        output.push_str(continuation_indent);
        output.push_str(line);
    }
    output.push(')');
    output
}

fn print_object_entry(entry: &RainbowObjectEntry, continuation_indent: &str) -> String {
    format!(
        "{}: {}",
        print_object_key(&entry.key),
        print_value(&entry.value, continuation_indent)
    )
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
