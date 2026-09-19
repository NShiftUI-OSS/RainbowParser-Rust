#[derive(Clone, Debug, Default, PartialEq)]
pub struct RainbowDocument {
    pub uses: Vec<RainbowUseDeclaration>,
    pub nodes: Vec<RainbowNode>,
}

impl RainbowDocument {
    pub fn new(nodes: Vec<RainbowNode>) -> Self {
        Self {
            uses: Vec::new(),
            nodes,
        }
    }

    pub fn with_uses(uses: Vec<RainbowUseDeclaration>, nodes: Vec<RainbowNode>) -> Self {
        Self { uses, nodes }
    }

    pub fn contains_placeholders(&self) -> bool {
        self.uses.iter().any(RainbowUseDeclaration::has_placeholder)
            || self.nodes.iter().any(RainbowNode::has_placeholder)
    }
}

/// Name that is either a concrete identifier or a `#{Placeholder}` template hole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RainbowName {
    Ident(String),
    Placeholder(String),
}

impl RainbowName {
    pub fn ident(name: impl Into<String>) -> Self {
        Self::Ident(name.into())
    }

    pub fn placeholder(name: impl Into<String>) -> Self {
        Self::Placeholder(name.into())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Ident(name) | Self::Placeholder(name) => name,
        }
    }

    pub fn is_placeholder(&self) -> bool {
        matches!(self, Self::Placeholder(_))
    }

    pub fn render(&self) -> String {
        match self {
            Self::Ident(name) => name.clone(),
            Self::Placeholder(name) => format!("#{{{name}}}"),
        }
    }
}

impl From<&str> for RainbowName {
    fn from(value: &str) -> Self {
        Self::Ident(value.to_string())
    }
}

impl From<String> for RainbowName {
    fn from(value: String) -> Self {
        Self::Ident(value)
    }
}

impl PartialEq<&str> for RainbowName {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, Self::Ident(name) if name == *other)
    }
}

/// Leading `//` line comments attached to the following `use` / node / parameter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RainbowLeadingTrivia {
    /// Text after `//` (including a leading space when the source had one).
    pub comments: Vec<String>,
}

impl RainbowLeadingTrivia {
    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }
}

/// Top-level pin: concrete `use Name@1.2.3`, or template `use #{Name}` (no version in source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RainbowUseDeclaration {
    pub name: RainbowName,
    pub version: Option<String>,
    pub leading: RainbowLeadingTrivia,
}

impl RainbowUseDeclaration {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: RainbowName::ident(name),
            version: Some(version.into()),
            leading: RainbowLeadingTrivia::default(),
        }
    }

    pub fn pinned(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self::new(name, version)
    }

    pub fn template_name(placeholder: impl Into<String>) -> Self {
        Self {
            name: RainbowName::placeholder(placeholder),
            version: None,
            leading: RainbowLeadingTrivia::default(),
        }
    }

    pub fn has_placeholder(&self) -> bool {
        self.name.is_placeholder()
    }

    pub fn render(&self) -> String {
        match (&self.name, &self.version) {
            (name, Some(version)) => format!("use {}@{version}", name.render()),
            (name, None) => format!("use {}", name.render()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RainbowBlock {
    pub children: Vec<RainbowNode>,
}

impl RainbowBlock {
    pub fn new(children: Vec<RainbowNode>) -> Self {
        Self { children }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RainbowNode {
    pub name: RainbowName,
    pub parameters: Vec<RainbowParameter>,
    pub block: Option<RainbowBlock>,
    pub leading: RainbowLeadingTrivia,
}

impl RainbowNode {
    pub fn new(name: impl Into<RainbowName>) -> Self {
        Self {
            name: name.into(),
            parameters: Vec::new(),
            block: None,
            leading: RainbowLeadingTrivia::default(),
        }
    }

    pub fn with_parameters(
        name: impl Into<RainbowName>,
        parameters: Vec<RainbowParameter>,
    ) -> Self {
        Self {
            name: name.into(),
            parameters,
            block: None,
            leading: RainbowLeadingTrivia::default(),
        }
    }

    pub fn with_children(
        name: impl Into<RainbowName>,
        parameters: Vec<RainbowParameter>,
        children: Vec<RainbowNode>,
    ) -> Self {
        Self {
            name: name.into(),
            parameters,
            block: Some(RainbowBlock::new(children)),
            leading: RainbowLeadingTrivia::default(),
        }
    }

    pub fn has_block(&self) -> bool {
        self.block.is_some()
    }

    pub fn children(&self) -> &[RainbowNode] {
        self.block
            .as_ref()
            .map(|block| block.children.as_slice())
            .unwrap_or(&[])
    }

    pub fn has_placeholder(&self) -> bool {
        self.name.is_placeholder()
            || self
                .parameters
                .iter()
                .any(|parameter| parameter.value.has_placeholder())
            || self.children().iter().any(RainbowNode::has_placeholder)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RainbowParameter {
    pub name: String,
    pub value: RainbowValue,
    pub leading: RainbowLeadingTrivia,
}

impl RainbowParameter {
    pub fn new(name: impl Into<String>, value: RainbowValue) -> Self {
        Self {
            name: name.into(),
            value,
            leading: RainbowLeadingTrivia::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RainbowObjectEntry {
    pub key: String,
    pub value: RainbowValue,
}

impl RainbowObjectEntry {
    pub fn new(key: impl Into<String>, value: RainbowValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

/// Allowlisted languages for `@LANG(...)` parameter values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EmbeddedLanguage {
    Json,
    Yaml,
    Xml,
    Html,
    Markdown,
}

impl EmbeddedLanguage {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "JSON" => Some(Self::Json),
            "YAML" => Some(Self::Yaml),
            "XML" => Some(Self::Xml),
            "HTML" => Some(Self::Html),
            "MARKDOWN" => Some(Self::Markdown),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Xml => "XML",
            Self::Html => "HTML",
            Self::Markdown => "MARKDOWN",
        }
    }
}

/// Body of a `@LANG(...)` value: raw language text or a whole-blob placeholder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RainbowTaggedBody {
    Text(String),
    Placeholder(String),
}

impl RainbowTaggedBody {
    pub fn has_placeholder(&self) -> bool {
        matches!(self, Self::Placeholder(_))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RainbowValue {
    String(String),
    Int(i64),
    Double(f64),
    Bool(bool),
    Identifier(String),
    Placeholder(String),
    /// `@LANG(...)` parameter payload (raw language text or `#{blob}`).
    Tagged {
        language: EmbeddedLanguage,
        body: RainbowTaggedBody,
        body_range: crate::diagnostics::RainbowSourceRange,
    },
    Array(Vec<RainbowValue>),
    Object(Vec<RainbowObjectEntry>),
    Null,
}

impl RainbowValue {
    pub fn has_placeholder(&self) -> bool {
        match self {
            Self::Placeholder(_) => true,
            Self::Tagged { body, .. } => body.has_placeholder(),
            Self::Array(values) => values.iter().any(Self::has_placeholder),
            Self::Object(entries) => entries.iter().any(|entry| entry.value.has_placeholder()),
            _ => false,
        }
    }
}
