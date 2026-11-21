use crate::ast::{
    RainbowBlock, RainbowDocument, RainbowLeadingTrivia, RainbowName, RainbowNode,
    RainbowObjectEntry, RainbowParameter, RainbowSlot, RainbowTrigger, RainbowUseDeclaration,
    RainbowValue,
};
use crate::diagnostics::{RainbowDiagnostic, RainbowParseError};
use crate::lexer::{RainbowToken, RainbowTokenKind};

pub fn parse_tokens(tokens: &[RainbowToken]) -> Result<RainbowDocument, RainbowParseError> {
    if tokens.is_empty() {
        return Ok(RainbowDocument::default());
    }

    RainbowSyntaxParser::new(tokens).parse()
}

struct RainbowSyntaxParser<'tokens> {
    tokens: &'tokens [RainbowToken],
    current: usize,
    diagnostics: Vec<RainbowDiagnostic>,
}

impl<'tokens> RainbowSyntaxParser<'tokens> {
    fn new(tokens: &'tokens [RainbowToken]) -> Self {
        Self {
            tokens,
            current: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse(mut self) -> Result<RainbowDocument, RainbowParseError> {
        let mut uses = Vec::new();
        let mut nodes = Vec::new();

        while !self.is_at_end() {
            let leading = self.take_leading_trivia();
            if matches!(&self.peek().kind, RainbowTokenKind::Identifier(name) if name == "use") {
                if let Some(mut declaration) = self.parse_use_declaration() {
                    declaration.leading = leading;
                    uses.push(declaration);
                } else {
                    self.synchronize_node();
                }
            } else if let Some(mut node) = self.parse_node() {
                node.leading = leading;
                nodes.push(node);
            } else {
                self.synchronize_node();
            }
        }

        if self.diagnostics.is_empty() {
            Ok(RainbowDocument::with_uses(uses, nodes))
        } else {
            Err(RainbowParseError::new(self.diagnostics))
        }
    }

    fn take_leading_trivia(&mut self) -> RainbowLeadingTrivia {
        let mut comments = Vec::new();
        while let RainbowTokenKind::Comment(text) = &self.peek().kind {
            comments.push(text.clone());
            self.advance();
        }
        RainbowLeadingTrivia { comments }
    }

    fn parse_use_declaration(&mut self) -> Option<RainbowUseDeclaration> {
        match &self.peek().kind {
            RainbowTokenKind::Identifier(name) if name == "use" => {}
            _ => return None,
        }
        self.advance();

        match &self.peek().kind {
            RainbowTokenKind::Placeholder(name) => {
                let name = name.clone();
                self.advance();
                if self.check(TokenPredicate::At) {
                    self.append_unexpected_token(
                        "Template use pins cannot mix '#{Name}' with '@version'; use `use #{Name}` or `use Name@1.0.0`.",
                    );
                    return None;
                }
                Some(RainbowUseDeclaration::template_name(name))
            }
            RainbowTokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();

                if !self.match_kind(TokenPredicate::At) {
                    self.append_unexpected_token(
                        "Expected '@MAJOR.MINOR.PATCH' after use pin name. \
Unversioned `use Name` is not allowed; use `use Name@1.0.0` or template `use #{Name}` \
(expand map value must include the version, e.g. `PromoBanner@1.0.0`).",
                    );
                    return None;
                }

                let version = match &self.peek().kind {
                    RainbowTokenKind::Version(version) => version.clone(),
                    _ => {
                        self.append_unexpected_token(
                            "Expected SemVer MAJOR.MINOR.PATCH after '@'.",
                        );
                        return None;
                    }
                };
                self.advance();
                Some(RainbowUseDeclaration::pinned(name, version))
            }
            _ => {
                self.append_unexpected_token(
                    "Expected plugin/event name or '#{Name}' after 'use'.",
                );
                None
            }
        }
    }

    fn parse_node(&mut self) -> Option<RainbowNode> {
        let name = match &self.peek().kind {
            RainbowTokenKind::Identifier(name) => RainbowName::ident(name.clone()),
            RainbowTokenKind::Placeholder(name) => RainbowName::placeholder(name.clone()),
            _ => {
                self.append_unexpected_token("Expected node name.");
                self.advance();
                return None;
            }
        };
        self.advance();

        let had_parentheses = self.match_kind(TokenPredicate::LeftParen);
        let parameters = if had_parentheses {
            self.parse_arguments()
        } else {
            Vec::new()
        };

        let mut anchor = self.previous_end_line();
        let block = if self.check(TokenPredicate::LeftBrace) {
            if !had_parentheses {
                self.diagnostics.push(RainbowDiagnostic::error(
                    "rainbow.parser.childrenBrace",
                    "A children block belongs after '()', on the same line as ')'.",
                    self.peek().range,
                ));
            }
            if self.peek_line() != anchor {
                self.diagnostics.push(RainbowDiagnostic::error(
                    "rainbow.parser.childrenBrace",
                    "The children block must open on the same line as ')'.",
                    self.peek().range,
                ));
            }
            self.advance();
            let block = self.parse_block();
            anchor = self.previous_end_line();
            Some(block)
        } else {
            None
        };

        let mut node = RainbowNode {
            name,
            parameters,
            block,
            leading: RainbowLeadingTrivia::default(),
            id: None,
            style: Vec::new(),
            events: Vec::new(),
            slots: Vec::new(),
        };
        self.parse_modifiers(&mut node, anchor);
        Some(node)
    }

    /// View modifiers stay on the line immediately below the call or the previous modifier.
    fn parse_modifiers(&mut self, node: &mut RainbowNode, mut anchor: usize) {
        let mut seen = Vec::new();
        loop {
            if matches!(self.peek().kind, RainbowTokenKind::Comment(_)) {
                if self.comment_introduces_modifier() {
                    self.diagnostics.push(RainbowDiagnostic::error(
                        "rainbow.parser.modifierSpacing",
                        "A comment cannot sit between a plugin and its view modifier.",
                        self.peek().range,
                    ));
                    self.advance();
                    continue;
                }
                break;
            }

            let RainbowTokenKind::DotIdentifier(modifier) = &self.peek().kind else {
                break;
            };
            let modifier = modifier.clone();
            let line = self.peek_line();
            if !matches!(modifier.as_str(), "id" | "style" | "events" | "slots") {
                if line <= anchor.saturating_add(1) {
                    self.diagnostics.push(RainbowDiagnostic::error(
                        "rainbow.parser.unknownModifier",
                        format!("Unknown view modifier '.{modifier}'."),
                        self.peek().range,
                    ));
                    self.advance();
                    continue;
                }
                break;
            }

            if line == anchor {
                self.diagnostics.push(RainbowDiagnostic::error(
                    "rainbow.parser.modifierSpacing",
                    format!("'.{modifier}' must start on the line below the plugin."),
                    self.peek().range,
                ));
            } else if line > anchor.saturating_add(1) {
                self.diagnostics.push(RainbowDiagnostic::error(
                    "rainbow.parser.modifierSpacing",
                    format!("'.{modifier}' must be on the next line, with no blank line above it."),
                    self.peek().range,
                ));
            }

            if seen.contains(&modifier) {
                self.diagnostics.push(RainbowDiagnostic::error(
                    "rainbow.parser.duplicateModifier",
                    format!("'.{modifier}' can be written only once on a plugin."),
                    self.peek().range,
                ));
            }
            seen.push(modifier.clone());
            self.advance();

            match modifier.as_str() {
                "id" => node.id = self.parse_id_modifier(),
                "style" => node.style = self.parse_style_modifier(),
                "events" => node.events = self.parse_events_modifier(),
                "slots" => node.slots = self.parse_slots_modifier(),
                _ => {}
            }
            anchor = self.previous_end_line();
        }
    }

    fn parse_id_modifier(&mut self) -> Option<String> {
        self.consume(TokenPredicate::LeftParen, "Expected '(' after '.id'.");
        let id = match &self.peek().kind {
            RainbowTokenKind::String(value) => {
                let value = value.clone();
                self.advance();
                Some(value)
            }
            _ => {
                self.append_unexpected_token("Expected a string inside '.id(\"...\")'.");
                None
            }
        };
        self.consume(TokenPredicate::RightParen, "Expected ')' after '.id'.");
        id
    }

    fn parse_style_modifier(&mut self) -> Vec<RainbowParameter> {
        if !self.match_kind(TokenPredicate::LeftParen) {
            self.append_unexpected_token("Expected '(' after '.style'.");
            return Vec::new();
        }
        self.parse_arguments()
    }

    fn parse_events_modifier(&mut self) -> Vec<RainbowTrigger> {
        if !self.brace_on_same_line("'.events'") {
            return Vec::new();
        }
        self.advance();
        let mut triggers = Vec::new();
        while !self.significant_is(TokenPredicate::RightBrace) && !self.is_at_end() {
            let leading = self.take_leading_trivia();
            if let Some(mut trigger) = self.parse_trigger() {
                trigger.leading = leading;
                triggers.push(trigger);
            } else {
                self.synchronize_node();
            }
        }
        let _ = self.take_leading_trivia();
        self.consume(TokenPredicate::RightBrace, "Expected '}' after '.events'.");
        triggers
    }

    fn parse_trigger(&mut self) -> Option<RainbowTrigger> {
        let name = match &self.peek().kind {
            RainbowTokenKind::Identifier(name) => RainbowName::ident(name.clone()),
            _ => {
                self.append_unexpected_token("Expected a trigger name inside '.events'.");
                return None;
            }
        };
        self.advance();
        if self.check(TokenPredicate::LeftParen) {
            self.advance();
            let _ = self.parse_arguments();
        }
        if !self.brace_on_same_line("a trigger") {
            return None;
        }
        self.advance();
        let events = self.parse_event_calls();
        Some(RainbowTrigger {
            name,
            events,
            leading: RainbowLeadingTrivia::default(),
        })
    }

    fn parse_event_calls(&mut self) -> Vec<RainbowNode> {
        let mut events = Vec::new();
        while !self.significant_is(TokenPredicate::RightBrace) && !self.is_at_end() {
            let leading = self.take_leading_trivia();
            if let Some(mut event) = self.parse_node() {
                event.leading = leading;
                if event.block.is_some()
                    || !event.style.is_empty()
                    || !event.events.is_empty()
                    || !event.slots.is_empty()
                {
                    self.diagnostics.push(RainbowDiagnostic::error(
                        "rainbow.parser.eventShape",
                        "An event only accepts metadata in parentheses and an optional '.id'.",
                        self.previous().range,
                    ));
                }
                events.push(event);
            } else {
                self.synchronize_node();
            }
        }
        let _ = self.take_leading_trivia();
        self.consume(TokenPredicate::RightBrace, "Expected '}' after trigger.");
        events
    }

    fn parse_slots_modifier(&mut self) -> Vec<RainbowSlot> {
        if !self.brace_on_same_line("'.slots'") {
            return Vec::new();
        }
        self.advance();
        let mut slots = Vec::new();
        while !self.significant_is(TokenPredicate::RightBrace) && !self.is_at_end() {
            let leading = self.take_leading_trivia();
            if let Some(mut slot) = self.parse_slot() {
                slot.leading = leading;
                slots.push(slot);
            } else {
                self.synchronize_node();
            }
        }
        let _ = self.take_leading_trivia();
        self.consume(TokenPredicate::RightBrace, "Expected '}' after '.slots'.");
        slots
    }

    fn parse_slot(&mut self) -> Option<RainbowSlot> {
        let name = match &self.peek().kind {
            RainbowTokenKind::Identifier(name) => RainbowName::ident(name.clone()),
            _ => {
                self.append_unexpected_token("Expected a slot name inside '.slots'.");
                return None;
            }
        };
        self.advance();
        if !self.brace_on_same_line("a slot") {
            return None;
        }
        self.advance();
        let plugins = self.parse_plugin_list();
        Some(RainbowSlot {
            name,
            plugins,
            leading: RainbowLeadingTrivia::default(),
        })
    }

    fn parse_plugin_list(&mut self) -> Vec<RainbowNode> {
        let mut plugins = Vec::new();
        while !self.significant_is(TokenPredicate::RightBrace) && !self.is_at_end() {
            let leading = self.take_leading_trivia();
            if let Some(mut plugin) = self.parse_node() {
                plugin.leading = leading;
                plugins.push(plugin);
            } else {
                self.synchronize_node();
            }
        }
        let _ = self.take_leading_trivia();
        self.consume(TokenPredicate::RightBrace, "Expected '}' after plugins.");
        plugins
    }

    fn brace_on_same_line(&mut self, what: &str) -> bool {
        if !self.check(TokenPredicate::LeftBrace) {
            self.append_unexpected_token(format!("Expected '{{' on the same line as {what}."));
            return false;
        }
        if self.peek_line() != self.previous_end_line() {
            self.diagnostics.push(RainbowDiagnostic::error(
                "rainbow.parser.braceLine",
                format!("The '{{' of {what} must stay on the same line as its name."),
                self.peek().range,
            ));
        }
        true
    }

    fn comment_introduces_modifier(&self) -> bool {
        let mut index = self.current;
        while let Some(token) = self.tokens.get(index) {
            match &token.kind {
                RainbowTokenKind::Comment(_) => index += 1,
                RainbowTokenKind::DotIdentifier(name)
                    if matches!(name.as_str(), "id" | "style" | "events" | "slots") =>
                {
                    return true;
                }
                _ => return false,
            }
        }
        false
    }

    fn peek_line(&self) -> usize {
        self.peek().range.start.line
    }

    fn previous_end_line(&self) -> usize {
        self.previous().range.end.line
    }

    fn parse_arguments(&mut self) -> Vec<RainbowParameter> {
        let mut parameters = Vec::new();

        if self.significant_is(TokenPredicate::RightParen) {
            let _ = self.take_leading_trivia();
            self.advance();
            return parameters;
        }

        loop {
            if self.significant_is(TokenPredicate::RightParen) || self.is_at_end() {
                break;
            }

            if let Some(parameter) = self.parse_parameter() {
                parameters.push(parameter);
            } else {
                self.synchronize_parameter();
            }

            if !self.match_kind(TokenPredicate::Comma) {
                break;
            }

            if self.significant_is(TokenPredicate::RightParen) {
                self.diagnostics.push(RainbowDiagnostic::error(
                    "rainbow.parser.unexpectedToken",
                    "Trailing comma is not allowed after the last parameter.",
                    self.previous().range,
                ));
                break;
            }
        }

        let _ = self.take_leading_trivia();
        self.consume(
            TokenPredicate::RightParen,
            "Expected ')' after parameter list.",
        );
        parameters
    }

    fn parse_parameter(&mut self) -> Option<RainbowParameter> {
        let leading = self.take_leading_trivia();
        let name = match &self.peek().kind {
            RainbowTokenKind::Identifier(name) => name.clone(),
            _ => {
                self.append_unexpected_token("Expected parameter name.");
                return None;
            }
        };
        self.advance();

        self.consume(TokenPredicate::Colon, "Expected ':' after parameter name.");

        let Some(value) = self.parse_value() else {
            self.synchronize_parameter();
            return None;
        };

        Some(RainbowParameter {
            name,
            value,
            leading,
        })
    }

    fn parse_block(&mut self) -> RainbowBlock {
        let mut children = Vec::new();

        while !self.significant_is(TokenPredicate::RightBrace) && !self.is_at_end() {
            let leading = self.take_leading_trivia();
            if let Some(mut child) = self.parse_node() {
                child.leading = leading;
                children.push(child);
            } else {
                self.synchronize_node();
            }
        }

        let _ = self.take_leading_trivia();
        self.consume(TokenPredicate::RightBrace, "Expected '}' after block.");
        RainbowBlock::new(children)
    }

    fn parse_value(&mut self) -> Option<RainbowValue> {
        match &self.peek().kind {
            RainbowTokenKind::String(value) => {
                let value = value.clone();
                self.advance();
                Some(RainbowValue::String(value))
            }
            RainbowTokenKind::Int(value) => {
                let value = *value;
                self.advance();
                Some(RainbowValue::Int(value))
            }
            RainbowTokenKind::Double(value) => {
                let value = *value;
                self.advance();
                Some(RainbowValue::Double(value))
            }
            RainbowTokenKind::Bool(value) => {
                let value = *value;
                self.advance();
                Some(RainbowValue::Bool(value))
            }
            RainbowTokenKind::Null => {
                self.advance();
                Some(RainbowValue::Null)
            }
            RainbowTokenKind::DotIdentifier(value) => {
                let value = value.clone();
                self.advance();
                Some(RainbowValue::Identifier(value))
            }
            RainbowTokenKind::Placeholder(value) => {
                let value = value.clone();
                self.advance();
                Some(RainbowValue::Placeholder(value))
            }
            RainbowTokenKind::Tagged {
                language,
                body,
                body_range,
            } => {
                let language = *language;
                let body = body.clone();
                let body_range = *body_range;
                self.advance();
                Some(RainbowValue::Tagged {
                    language,
                    body,
                    body_range,
                })
            }
            RainbowTokenKind::Identifier(value) => {
                let name = value.clone();
                self.append_unexpected_token(format!(
                    "Enum identifier values must start with '.'; did you mean '.{name}'?"
                ));
                self.advance();
                None
            }
            RainbowTokenKind::LeftBracket => {
                self.advance();
                Some(self.parse_array())
            }
            RainbowTokenKind::LeftParen => {
                self.advance();
                Some(self.parse_object())
            }
            RainbowTokenKind::LeftBrace => {
                self.advance();
                Some(RainbowValue::Plugins(self.parse_plugin_list()))
            }
            _ => {
                self.append_unexpected_token("Expected value.");
                None
            }
        }
    }

    fn parse_array(&mut self) -> RainbowValue {
        let mut values = Vec::new();

        if self.match_kind(TokenPredicate::RightBracket) {
            return RainbowValue::Array(values);
        }

        loop {
            if self.check(TokenPredicate::RightBracket) || self.check(TokenPredicate::Eof) {
                break;
            }

            if let Some(value) = self.parse_value() {
                values.push(value);
            } else {
                self.synchronize_value();
            }

            if !self.match_kind(TokenPredicate::Comma) {
                break;
            }
        }

        self.consume(TokenPredicate::RightBracket, "Expected ']' after array.");
        RainbowValue::Array(values)
    }

    fn parse_object(&mut self) -> RainbowValue {
        let mut entries = Vec::new();

        if self.match_kind(TokenPredicate::RightParen) {
            return RainbowValue::Object(entries);
        }

        loop {
            if self.check(TokenPredicate::RightParen) || self.check(TokenPredicate::Eof) {
                break;
            }

            let Some(key) = self.parse_object_key() else {
                self.synchronize_value();
                if !self.match_kind(TokenPredicate::Comma) {
                    break;
                }
                continue;
            };

            self.consume(TokenPredicate::Colon, "Expected ':' after object key.");

            let Some(value) = self.parse_value() else {
                self.synchronize_value();
                if !self.match_kind(TokenPredicate::Comma) {
                    break;
                }
                continue;
            };

            entries.push(RainbowObjectEntry::new(key, value));

            if !self.match_kind(TokenPredicate::Comma) {
                break;
            }

            if self.check(TokenPredicate::RightParen) {
                self.diagnostics.push(RainbowDiagnostic::error(
                    "rainbow.parser.unexpectedToken",
                    "Trailing comma is not allowed after the last object entry.",
                    self.previous().range,
                ));
                break;
            }
        }

        self.consume(TokenPredicate::RightParen, "Expected ')' after object.");
        RainbowValue::Object(entries)
    }

    fn parse_object_key(&mut self) -> Option<String> {
        match &self.peek().kind {
            RainbowTokenKind::Identifier(key) | RainbowTokenKind::String(key) => {
                let key = key.clone();
                self.advance();
                Some(key)
            }
            _ => {
                self.append_unexpected_token("Expected object key.");
                None
            }
        }
    }

    fn consume(&mut self, predicate: TokenPredicate, message: &str) -> Option<RainbowToken> {
        if self.check(predicate) {
            return Some(self.advance().clone());
        }

        self.append_unexpected_token(message);
        None
    }

    fn match_kind(&mut self, predicate: TokenPredicate) -> bool {
        if !self.check(predicate) {
            return false;
        }

        self.advance();
        true
    }

    fn check(&self, predicate: TokenPredicate) -> bool {
        predicate.matches(&self.peek().kind)
    }

    /// True when the next non-comment token matches `predicate`.
    fn significant_is(&self, predicate: TokenPredicate) -> bool {
        let mut index = self.current;
        while index < self.tokens.len() {
            match &self.tokens[index].kind {
                RainbowTokenKind::Comment(_) => index += 1,
                kind => return predicate.matches(kind),
            }
        }
        false
    }

    fn advance(&mut self) -> &RainbowToken {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, RainbowTokenKind::Eof)
    }

    fn peek(&self) -> &RainbowToken {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &RainbowToken {
        &self.tokens[self.current - 1]
    }

    fn synchronize_node(&mut self) {
        while !self.is_at_end() {
            if self.check(TokenPredicate::RightBrace)
                || self.is_node_start()
                || matches!(&self.peek().kind, RainbowTokenKind::Identifier(name) if name == "use")
            {
                return;
            }

            self.advance();
        }
    }

    fn synchronize_parameter(&mut self) {
        while !self.is_at_end() {
            if self.check(TokenPredicate::Comma) || self.check(TokenPredicate::RightParen) {
                return;
            }

            self.advance();
        }
    }

    fn synchronize_value(&mut self) {
        while !self.is_at_end() {
            if self.check(TokenPredicate::Comma)
                || self.check(TokenPredicate::RightBracket)
                || self.check(TokenPredicate::RightBrace)
                || self.check(TokenPredicate::RightParen)
            {
                return;
            }

            self.advance();
        }
    }

    fn is_node_start(&self) -> bool {
        matches!(
            self.peek().kind,
            RainbowTokenKind::Identifier(_) | RainbowTokenKind::Placeholder(_)
        )
    }

    fn append_unexpected_token(&mut self, message: impl Into<String>) {
        self.diagnostics.push(RainbowDiagnostic::error(
            "rainbow.parser.unexpectedToken",
            message,
            self.peek().range,
        ));
    }
}

#[derive(Clone, Copy)]
enum TokenPredicate {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    RightBracket,
    Colon,
    Comma,
    At,
    Eof,
}

impl TokenPredicate {
    fn matches(self, kind: &RainbowTokenKind) -> bool {
        matches!(
            (self, kind),
            (Self::LeftParen, RainbowTokenKind::LeftParen)
                | (Self::RightParen, RainbowTokenKind::RightParen)
                | (Self::LeftBrace, RainbowTokenKind::LeftBrace)
                | (Self::RightBrace, RainbowTokenKind::RightBrace)
                | (Self::RightBracket, RainbowTokenKind::RightBracket)
                | (Self::Colon, RainbowTokenKind::Colon)
                | (Self::Comma, RainbowTokenKind::Comma)
                | (Self::At, RainbowTokenKind::At)
                | (Self::Eof, RainbowTokenKind::Eof)
        )
    }
}
