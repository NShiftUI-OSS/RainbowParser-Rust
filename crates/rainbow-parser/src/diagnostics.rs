use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RainbowSourceLocation {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl RainbowSourceLocation {
    pub const fn new(offset: usize, line: usize, column: usize) -> Self {
        Self {
            offset,
            line,
            column,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RainbowSourceRange {
    pub start: RainbowSourceLocation,
    pub end: RainbowSourceLocation,
}

impl RainbowSourceRange {
    pub const fn new(start: RainbowSourceLocation, end: RainbowSourceLocation) -> Self {
        Self { start, end }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RainbowDiagnosticSeverity {
    Error,
    Warning,
}

impl RainbowDiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RainbowDiagnostic {
    pub code: String,
    pub message: String,
    pub severity: RainbowDiagnosticSeverity,
    pub range: RainbowSourceRange,
}

impl RainbowDiagnostic {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        range: RainbowSourceRange,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: RainbowDiagnosticSeverity::Error,
            range,
        }
    }
}

impl Display for RainbowDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} at {}:{}: {}",
            self.severity.as_str().to_uppercase(),
            self.code,
            self.range.start.line,
            self.range.start.column,
            self.message
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RainbowParseError {
    pub diagnostics: Vec<RainbowDiagnostic>,
}

impl RainbowParseError {
    pub fn new(diagnostics: Vec<RainbowDiagnostic>) -> Self {
        Self { diagnostics }
    }

    pub fn single(diagnostic: RainbowDiagnostic) -> Self {
        Self {
            diagnostics: vec![diagnostic],
        }
    }
}

impl Display for RainbowParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                writeln!(formatter)?;
            }
            write!(formatter, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for RainbowParseError {}
