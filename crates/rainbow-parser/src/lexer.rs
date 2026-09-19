use crate::ast::{EmbeddedLanguage, RainbowTaggedBody};
use crate::diagnostics::{
    RainbowDiagnostic, RainbowParseError, RainbowSourceLocation, RainbowSourceRange,
};

#[derive(Clone, Debug, PartialEq)]
pub enum RainbowTokenKind {
    Identifier(String),
    /// Enum / bare value with required leading `.` in source (e.g. `.primary`).
    /// Payload is the name **without** the dot.
    DotIdentifier(String),
    String(String),
    Int(i64),
    Double(f64),
    /// SemVer core `MAJOR.MINOR.PATCH` (two or more dots), e.g. `1.0.0`.
    Version(String),
    Bool(bool),
    Null,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
    At,
    /// Template hole `#{Name}` — payload is the name inside the braces.
    Placeholder(String),
    /// `@LANG(...)` parameter value (raw language text or whole-blob placeholder).
    Tagged {
        language: EmbeddedLanguage,
        body: RainbowTaggedBody,
        body_range: RainbowSourceRange,
    },
    /// Line comment `//…` — payload is the text after `//` (may start with a space).
    Comment(String),
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RainbowToken {
    pub kind: RainbowTokenKind,
    pub range: RainbowSourceRange,
}

pub fn lex(source: &str) -> Result<Vec<RainbowToken>, RainbowParseError> {
    RainbowLexer::new(source).scan_tokens()
}

struct RainbowLexer {
    scanner: CharacterScanner,
    diagnostics: Vec<RainbowDiagnostic>,
}

impl RainbowLexer {
    fn new(source: &str) -> Self {
        Self {
            scanner: CharacterScanner::new(source),
            diagnostics: Vec::new(),
        }
    }

    fn scan_tokens(mut self) -> Result<Vec<RainbowToken>, RainbowParseError> {
        let mut tokens = Vec::new();

        while !self.scanner.is_at_end() {
            self.skip_whitespace();
            if self.scanner.is_at_end() {
                break;
            }

            let start = self.scanner.location();
            let character = self.scanner.advance().expect("scanner is not at end");

            match character {
                '(' => tokens.push(self.make_token(RainbowTokenKind::LeftParen, start)),
                ')' => tokens.push(self.make_token(RainbowTokenKind::RightParen, start)),
                '{' => tokens.push(self.make_token(RainbowTokenKind::LeftBrace, start)),
                '}' => tokens.push(self.make_token(RainbowTokenKind::RightBrace, start)),
                '[' => tokens.push(self.make_token(RainbowTokenKind::LeftBracket, start)),
                ']' => tokens.push(self.make_token(RainbowTokenKind::RightBracket, start)),
                ':' => tokens.push(self.make_token(RainbowTokenKind::Colon, start)),
                ',' => tokens.push(self.make_token(RainbowTokenKind::Comma, start)),
                '/' => {
                    if self.scanner.peek() == Some('/') {
                        self.scanner.advance();
                        let mut text = String::new();
                        while let Some(next) = self.scanner.peek() {
                            if next == '\n' {
                                break;
                            }
                            text.push(self.scanner.advance().expect("peeked"));
                        }
                        if text.ends_with('\r') {
                            text.pop();
                        }
                        tokens.push(self.make_token(RainbowTokenKind::Comment(text), start));
                    } else {
                        self.append_unexpected_character('/', start);
                    }
                }
                '@' => {
                    if let Some(token) = self.try_scan_tagged(start) {
                        tokens.push(token);
                    } else {
                        tokens.push(self.make_token(RainbowTokenKind::At, start));
                    }
                }
                '#' => {
                    if let Some(token) = self.scan_placeholder(start) {
                        tokens.push(token);
                    }
                }
                '.' => {
                    if self.scanner.peek().is_some_and(is_rainbow_identifier_start) {
                        tokens.push(self.scan_dot_identifier(start));
                    } else {
                        self.append_unexpected_character(character, start);
                    }
                }
                '"' => {
                    if let Some(token) = self.scan_string(start) {
                        tokens.push(token);
                    }
                }
                '-' => {
                    if self.scanner.peek().is_some_and(is_rainbow_digit) {
                        tokens.push(self.scan_number(start, character));
                    } else {
                        self.append_unexpected_character(character, start);
                    }
                }
                _ if is_rainbow_digit(character) => {
                    tokens.push(self.scan_number(start, character));
                }
                _ if is_rainbow_identifier_start(character) => {
                    tokens.push(self.scan_identifier(start, character));
                }
                _ => self.append_unexpected_character(character, start),
            }
        }

        let eof_location = self.scanner.location();
        tokens.push(RainbowToken {
            kind: RainbowTokenKind::Eof,
            range: RainbowSourceRange::new(eof_location, eof_location),
        });

        if self.diagnostics.is_empty() {
            Ok(tokens)
        } else {
            Err(RainbowParseError::new(self.diagnostics))
        }
    }

    fn skip_whitespace(&mut self) {
        while self.scanner.peek().is_some_and(is_rainbow_whitespace) {
            self.scanner.advance();
        }
    }

    fn make_token(&self, kind: RainbowTokenKind, start: RainbowSourceLocation) -> RainbowToken {
        RainbowToken {
            kind,
            range: RainbowSourceRange::new(start, self.scanner.location()),
        }
    }

    fn scan_string(&mut self, start: RainbowSourceLocation) -> Option<RainbowToken> {
        let mut value = String::new();

        while let Some(character) = self.scanner.peek() {
            if character == '"' {
                self.scanner.advance();
                return Some(self.make_token(RainbowTokenKind::String(value), start));
            }

            if character == '\n' || character == '\r' {
                self.append_diagnostic(
                    "rainbow.lexer.unterminatedString",
                    "Unterminated string literal.",
                    start,
                    self.scanner.location(),
                );
                return None;
            }

            if character == '\\' {
                self.scanner.advance();

                let Some(escaped) = self.scanner.advance() else {
                    self.append_diagnostic(
                        "rainbow.lexer.unterminatedString",
                        "Unterminated string literal.",
                        start,
                        self.scanner.location(),
                    );
                    return None;
                };

                match escaped {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    '0' => value.push('\0'),
                    _ => self.append_diagnostic(
                        "rainbow.lexer.invalidEscape",
                        format!("Invalid escape sequence \\{escaped}."),
                        start,
                        self.scanner.location(),
                    ),
                }
            } else {
                value.push(character);
                self.scanner.advance();
            }
        }

        self.append_diagnostic(
            "rainbow.lexer.unterminatedString",
            "Unterminated string literal.",
            start,
            self.scanner.location(),
        );
        None
    }

    fn scan_number(&mut self, start: RainbowSourceLocation, first: char) -> RainbowToken {
        let mut text = first.to_string();
        let mut dot_count = 0usize;

        while self.scanner.peek().is_some_and(is_rainbow_digit) {
            text.push(self.scanner.advance().expect("peeked digit"));
        }

        while self.scanner.peek() == Some('.')
            && self.scanner.peek_next().is_some_and(is_rainbow_digit)
        {
            dot_count += 1;
            text.push('.');
            self.scanner.advance();

            while self.scanner.peek().is_some_and(is_rainbow_digit) {
                text.push(self.scanner.advance().expect("peeked digit"));
            }
        }

        if dot_count >= 2 {
            return self.make_token(RainbowTokenKind::Version(text), start);
        }

        if dot_count == 1 {
            return match text.parse::<f64>() {
                Ok(value) if value.is_finite() => {
                    self.make_token(RainbowTokenKind::Double(value), start)
                }
                _ => {
                    self.append_invalid_number(&text, start);
                    self.make_token(RainbowTokenKind::Double(0.0), start)
                }
            };
        }

        match text.parse::<i64>() {
            Ok(value) => self.make_token(RainbowTokenKind::Int(value), start),
            Err(_) => {
                self.append_invalid_number(&text, start);
                self.make_token(RainbowTokenKind::Int(0), start)
            }
        }
    }

    fn scan_identifier(&mut self, start: RainbowSourceLocation, first: char) -> RainbowToken {
        let mut text = first.to_string();

        while self
            .scanner
            .peek()
            .is_some_and(is_rainbow_identifier_continuation)
        {
            text.push(
                self.scanner
                    .advance()
                    .expect("peeked identifier continuation"),
            );
        }

        self.make_token(
            match text.as_str() {
                "true" => RainbowTokenKind::Bool(true),
                "false" => RainbowTokenKind::Bool(false),
                "null" => RainbowTokenKind::Null,
                _ => RainbowTokenKind::Identifier(text),
            },
            start,
        )
    }

    fn scan_dot_identifier(&mut self, start: RainbowSourceLocation) -> RainbowToken {
        let first = self
            .scanner
            .advance()
            .expect("peeked identifier start after '.'");
        let mut text = first.to_string();

        while self
            .scanner
            .peek()
            .is_some_and(is_rainbow_identifier_continuation)
        {
            text.push(
                self.scanner
                    .advance()
                    .expect("peeked identifier continuation"),
            );
        }

        self.make_token(RainbowTokenKind::DotIdentifier(text), start)
    }

    fn scan_placeholder(&mut self, start: RainbowSourceLocation) -> Option<RainbowToken> {
        match self.scan_placeholder_name(start) {
            Some(text) => Some(self.make_token(RainbowTokenKind::Placeholder(text), start)),
            None => None,
        }
    }

    /// After `@` has been consumed: try `@LANG(...)`. Restores scanner on mismatch.
    fn try_scan_tagged(&mut self, start: RainbowSourceLocation) -> Option<RainbowToken> {
        let checkpoint = self.scanner.checkpoint();

        let Some(first) = self
            .scanner
            .peek()
            .filter(|c| is_rainbow_identifier_start(*c))
        else {
            return None;
        };
        self.scanner.advance();
        let mut language_name = first.to_string();
        while self
            .scanner
            .peek()
            .is_some_and(is_rainbow_identifier_continuation)
        {
            language_name.push(
                self.scanner
                    .advance()
                    .expect("peeked identifier continuation"),
            );
        }

        if self.scanner.peek() != Some('(') {
            self.scanner.restore(checkpoint);
            return None;
        }
        self.scanner.advance();

        let Some(language) = EmbeddedLanguage::parse(&language_name) else {
            self.append_diagnostic(
                "rainbow.lexer.unknownEmbeddedLanguage",
                format!(
                    "Unknown embedded language '@{language_name}'. \
Expected JSON, YAML, XML, HTML, or MARKDOWN."
                ),
                start,
                self.scanner.location(),
            );
            let _ = self.scan_tagged_raw_body(start, EmbeddedLanguage::Json);
            return None;
        };

        let body_start = self.scanner.location();
        if self.scanner.peek() == Some('#') && self.scanner.peek_next() == Some('{') {
            let placeholder_start = body_start;
            self.scanner.advance(); // #
            match self.scan_placeholder_name(placeholder_start) {
                Some(name) => {
                    let body_end = self.scanner.location();
                    if self.scanner.peek() != Some(')') {
                        self.append_diagnostic(
                            "rainbow.lexer.unterminatedTagged",
                            format!("Expected ')' after '@{}(#{{{name}}})'.", language.as_str()),
                            start,
                            self.scanner.location(),
                        );
                        return None;
                    }
                    self.scanner.advance();
                    return Some(RainbowToken {
                        kind: RainbowTokenKind::Tagged {
                            language,
                            body: RainbowTaggedBody::Placeholder(name),
                            body_range: RainbowSourceRange::new(body_start, body_end),
                        },
                        range: RainbowSourceRange::new(start, self.scanner.location()),
                    });
                }
                None => return None,
            }
        }

        let Some((text, body_end)) = self.scan_tagged_raw_body(start, language) else {
            return None;
        };

        Some(RainbowToken {
            kind: RainbowTokenKind::Tagged {
                language,
                body: RainbowTaggedBody::Text(text),
                body_range: RainbowSourceRange::new(body_start, body_end),
            },
            range: RainbowSourceRange::new(start, self.scanner.location()),
        })
    }

    /// Scan `#{Name}` after `#` has **not** been consumed yet when called from
    /// placeholder path; for tagged path `#` is already consumed and `{` must follow.
    fn scan_placeholder_name(&mut self, start: RainbowSourceLocation) -> Option<String> {
        // Caller for `#` path: `#` already consumed; expect `{`.
        // Caller for tagged: `#` already consumed; expect `{`.
        if self.scanner.peek() != Some('{') {
            // Standalone `#` path uses append_unexpected; tagged already ate `#`.
            if self.scanner.location().offset == start.offset + 1 {
                // tagged: we advanced past `#` from body_start
            }
            self.append_diagnostic(
                "rainbow.lexer.invalidPlaceholder",
                "Expected placeholder name after '#{'.",
                start,
                self.scanner.location(),
            );
            return None;
        }
        self.scanner.advance();

        let Some(first) = self
            .scanner
            .peek()
            .filter(|c| is_rainbow_identifier_start(*c))
        else {
            self.append_diagnostic(
                "rainbow.lexer.invalidPlaceholder",
                "Expected placeholder name after '#{'.",
                start,
                self.scanner.location(),
            );
            return None;
        };
        self.scanner.advance();
        let mut text = first.to_string();

        while self
            .scanner
            .peek()
            .is_some_and(is_rainbow_identifier_continuation)
        {
            text.push(
                self.scanner
                    .advance()
                    .expect("peeked identifier continuation"),
            );
        }

        if self.scanner.peek() != Some('}') {
            self.append_diagnostic(
                "rainbow.lexer.unterminatedPlaceholder",
                "Unterminated placeholder; expected '}'.",
                start,
                self.scanner.location(),
            );
            return None;
        }
        self.scanner.advance();
        Some(text)
    }

    /// Scan raw embedded payload until the matching `)` (already past opening `(`).
    /// Returns `(body_text, body_end_location)` and consumes the closing `)`.
    fn scan_tagged_raw_body(
        &mut self,
        tag_start: RainbowSourceLocation,
        language: EmbeddedLanguage,
    ) -> Option<(String, RainbowSourceLocation)> {
        let mut depth = 1usize;
        let mut body = String::new();
        let mut in_string: Option<char> = None;
        let mut escape = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut in_cdata = false;
        let mut in_md_fence = false;

        let allow_yaml_hash_comments = matches!(language, EmbeddedLanguage::Yaml);
        let allow_xml_markup = matches!(language, EmbeddedLanguage::Xml | EmbeddedLanguage::Html);
        let allow_single_quotes = !matches!(language, EmbeddedLanguage::Json);
        let markdown = matches!(language, EmbeddedLanguage::Markdown);

        while !self.scanner.is_at_end() {
            let Some(character) = self.scanner.peek() else {
                break;
            };

            // Reject partial `#{...}` interpolation inside raw payloads.
            if in_string.is_none()
                && !in_line_comment
                && !in_block_comment
                && !in_cdata
                && character == '#'
                && self.scanner.peek_next() == Some('{')
            {
                let start = self.scanner.location();
                self.append_diagnostic(
                    "rainbow.lexer.embeddedPartialPlaceholder",
                    "Partial '#{...}' interpolation is not allowed inside @LANG(...); \
use '@LANG(#{Name})' for a whole-blob placeholder.",
                    start,
                    self.scanner.location(),
                );
                return None;
            }

            if markdown && in_string.is_none() && !in_md_fence && self.starts_with("```") {
                for _ in 0..3 {
                    body.push(self.scanner.advance().expect("fence"));
                }
                in_md_fence = true;
                continue;
            }
            if markdown && in_md_fence {
                if self.starts_with("```") {
                    for _ in 0..3 {
                        body.push(self.scanner.advance().expect("fence close"));
                    }
                    in_md_fence = false;
                    continue;
                }
                // Still honor Rainbow paren balance so `@MARKDOWN(```…)` can close.
                if character == '(' {
                    depth += 1;
                    body.push(character);
                    self.scanner.advance();
                    continue;
                }
                if character == ')' {
                    depth -= 1;
                    if depth == 0 {
                        let body_end = self.scanner.location();
                        self.scanner.advance();
                        return Some((dedent_embedded_body(body), body_end));
                    }
                    body.push(character);
                    self.scanner.advance();
                    continue;
                }
                body.push(character);
                self.scanner.advance();
                continue;
            }

            if in_line_comment {
                body.push(character);
                self.scanner.advance();
                if character == '\n' {
                    in_line_comment = false;
                }
                continue;
            }

            if in_block_comment {
                body.push(character);
                self.scanner.advance();
                if character == '-'
                    && self.scanner.peek() == Some('-')
                    && self.scanner.peek_next() == Some('>')
                {
                    body.push(self.scanner.advance().expect("peeked -"));
                    body.push(self.scanner.advance().expect("peeked >"));
                    in_block_comment = false;
                }
                continue;
            }

            if in_cdata {
                body.push(character);
                self.scanner.advance();
                if character == ']'
                    && self.scanner.peek() == Some(']')
                    && self.scanner.peek_next() == Some('>')
                {
                    body.push(self.scanner.advance().expect("peeked ]"));
                    body.push(self.scanner.advance().expect("peeked >"));
                    in_cdata = false;
                }
                continue;
            }

            if let Some(quote) = in_string {
                body.push(character);
                self.scanner.advance();
                if escape {
                    escape = false;
                    continue;
                }
                if character == '\\' && quote == '"' {
                    escape = true;
                    continue;
                }
                if character == quote {
                    in_string = None;
                }
                continue;
            }

            if allow_xml_markup
                && character == '<'
                && self.scanner.peek_next() == Some('!')
                && self.peek_offset(2) == Some('-')
                && self.peek_offset(3) == Some('-')
            {
                for _ in 0..4 {
                    body.push(self.scanner.advance().expect("comment opener"));
                }
                in_block_comment = true;
                continue;
            }

            if allow_xml_markup && self.starts_with("<![CDATA[") {
                for _ in 0.."<![CDATA[".len() {
                    body.push(self.scanner.advance().expect("cdata opener"));
                }
                in_cdata = true;
                continue;
            }

            if allow_yaml_hash_comments && character == '#' {
                in_line_comment = true;
                body.push(character);
                self.scanner.advance();
                continue;
            }

            if character == '"' || (allow_single_quotes && character == '\'') {
                in_string = Some(character);
                body.push(character);
                self.scanner.advance();
                continue;
            }

            if character == '(' {
                depth += 1;
                body.push(character);
                self.scanner.advance();
                continue;
            }

            if character == ')' {
                depth -= 1;
                if depth == 0 {
                    let body_end = self.scanner.location();
                    self.scanner.advance();
                    return Some((dedent_embedded_body(body), body_end));
                }
                body.push(character);
                self.scanner.advance();
                continue;
            }

            body.push(character);
            self.scanner.advance();
        }

        self.append_diagnostic(
            "rainbow.lexer.unterminatedTagged",
            "Unterminated @LANG(...); expected closing ')'.",
            tag_start,
            self.scanner.location(),
        );
        None
    }

    fn peek_offset(&self, offset: usize) -> Option<char> {
        let mut index = self.scanner.index;
        for _ in 0..offset {
            index = self.scanner.next_index(index)?;
        }
        self.scanner.logical_char_at(index)
    }

    fn starts_with(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        let mut index = self.scanner.index;
        for (i, expected) in chars.iter().enumerate() {
            let Some(actual) = self.scanner.logical_char_at(index) else {
                return false;
            };
            if actual != *expected {
                return false;
            }
            if i + 1 == chars.len() {
                return true;
            }
            index = match self.scanner.next_index(index) {
                Some(next) => next,
                None => return false,
            };
        }
        true
    }

    fn append_unexpected_character(&mut self, character: char, start: RainbowSourceLocation) {
        self.append_diagnostic(
            "rainbow.lexer.unexpectedCharacter",
            format!("Unexpected character '{character}'."),
            start,
            self.scanner.location(),
        );
    }

    fn append_invalid_number(&mut self, text: &str, start: RainbowSourceLocation) {
        self.append_diagnostic(
            "rainbow.lexer.invalidNumber",
            format!("Invalid number literal '{text}'."),
            start,
            self.scanner.location(),
        );
    }

    fn append_diagnostic(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        start: RainbowSourceLocation,
        end: RainbowSourceLocation,
    ) {
        self.diagnostics.push(RainbowDiagnostic::error(
            code,
            message,
            RainbowSourceRange::new(start, end),
        ));
    }
}

/// Strip the common leading indent from continuation lines so formatted
/// `@LANG(...)` bodies round-trip without breaking YAML/JSON indentation.
fn dedent_embedded_body(body: String) -> String {
    if !body.contains('\n') {
        return body;
    }

    let lines: Vec<&str> = body.lines().collect();
    let min_indent = lines
        .iter()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|c| *c == ' ' || *c == '\t').count())
        .min();

    let Some(indent) = min_indent.filter(|value| *value > 0) else {
        return body;
    };

    let mut output = String::with_capacity(body.len());
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        if index == 0 || line.trim().is_empty() {
            output.push_str(line);
            continue;
        }
        let stripped: String = line.chars().skip(indent).collect();
        output.push_str(&stripped);
    }
    if body.ends_with('\n') {
        output.push('\n');
    }
    output
}

#[derive(Clone)]
struct CharacterScanner {
    characters: Vec<char>,
    index: usize,
    location: RainbowSourceLocation,
}

#[derive(Clone, Copy)]
struct ScannerCheckpoint {
    index: usize,
    location: RainbowSourceLocation,
}

impl CharacterScanner {
    fn new(source: &str) -> Self {
        Self {
            characters: source.chars().collect(),
            index: 0,
            location: RainbowSourceLocation::new(0, 1, 1),
        }
    }

    fn checkpoint(&self) -> ScannerCheckpoint {
        ScannerCheckpoint {
            index: self.index,
            location: self.location,
        }
    }

    fn restore(&mut self, checkpoint: ScannerCheckpoint) {
        self.index = checkpoint.index;
        self.location = checkpoint.location;
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.characters.len()
    }

    fn location(&self) -> RainbowSourceLocation {
        self.location
    }

    fn peek(&self) -> Option<char> {
        self.logical_char_at(self.index)
    }

    fn peek_next(&self) -> Option<char> {
        let next_index = self.next_index(self.index)?;
        self.logical_char_at(next_index)
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        // Byte length in the original UTF-8 source (CRLF counts as 2).
        let byte_len = if self.characters.get(self.index) == Some(&'\r')
            && self.characters.get(self.index + 1) == Some(&'\n')
        {
            2
        } else {
            character.len_utf8()
        };
        let consumed = if byte_len == 2
            && self.characters.get(self.index) == Some(&'\r')
            && self.characters.get(self.index + 1) == Some(&'\n')
        {
            2
        } else {
            1
        };
        self.index += consumed;

        if character == '\n' || character == '\r' {
            self.location = RainbowSourceLocation::new(
                self.location.offset + byte_len,
                self.location.line + 1,
                1,
            );
        } else {
            self.location = RainbowSourceLocation::new(
                self.location.offset + byte_len,
                self.location.line,
                self.location.column + 1,
            );
        }

        Some(character)
    }

    fn logical_char_at(&self, index: usize) -> Option<char> {
        let character = *self.characters.get(index)?;
        if character == '\r' && self.characters.get(index + 1) == Some(&'\n') {
            Some('\n')
        } else {
            Some(character)
        }
    }

    fn next_index(&self, index: usize) -> Option<usize> {
        if index >= self.characters.len() {
            return None;
        }

        let next = if self.characters.get(index) == Some(&'\r')
            && self.characters.get(index + 1) == Some(&'\n')
        {
            index + 2
        } else {
            index + 1
        };

        (next < self.characters.len()).then_some(next)
    }
}

fn is_rainbow_whitespace(character: char) -> bool {
    character == ' ' || character == '\n' || character == '\r' || character == '\t'
}

fn is_rainbow_digit(character: char) -> bool {
    character.is_ascii_digit()
}

fn is_rainbow_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_rainbow_identifier_continuation(character: char) -> bool {
    is_rainbow_identifier_start(character) || is_rainbow_digit(character)
}
