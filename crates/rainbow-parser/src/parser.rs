use crate::ast::{
    RainbowBlock, RainbowDocument, RainbowLeadingTrivia, RainbowName, RainbowNode,
    RainbowObjectEntry, RainbowParameter, RainbowUseDeclaration, RainbowValue,
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

        let parameters = if self.match_kind(TokenPredicate::LeftParen) {
            self.parse_arguments()
        } else {
            Vec::new()
        };

        let block = if self.match_kind(TokenPredicate::LeftBrace) {
            Some(self.parse_block())
        } else {
            None
        };

        Some(RainbowNode {
            name,
            parameters,
            block,
            leading: RainbowLeadingTrivia::default(),
        })
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
