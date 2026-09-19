use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rainbow_parser::{
    analyze_document_name_kinds, analyze_embedded_values, collect_embedded_regions, decode,
    decode_name_registry, encode, NameRegistry, RainbowDocument, RAINBOW_PARSER_VERSION,
};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::{ErrorCode, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::convert;
use crate::embedded_protocol::{self, EmbeddedRegionDto, EmbeddedRegionsParams};

#[derive(Debug)]
struct DocumentState {
    text: String,
    document: Option<RainbowDocument>,
}

#[derive(Debug, Default)]
struct RegistryState {
    path: Option<PathBuf>,
    uri: Option<Url>,
    registry: Option<NameRegistry>,
}

#[derive(Debug)]
pub struct RainbowLanguageServer {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
    registry: Arc<RwLock<RegistryState>>,
}

impl RainbowLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            registry: Arc::new(RwLock::new(RegistryState::default())),
        }
    }

    /// Custom request: list `@LANG(...)` bodies for bundled IDE language services.
    pub async fn embedded_regions(
        &self,
        params: EmbeddedRegionsParams,
    ) -> Result<Vec<EmbeddedRegionDto>> {
        let uri = embedded_protocol::document_uri(&params).clone();
        let documents = self.documents.read().await;
        let Some(state) = documents.get(&uri) else {
            return Err(tower_lsp::jsonrpc::Error {
                code: ErrorCode::InvalidParams,
                message: "Document not open in rainbow-lsp.".into(),
                data: None,
            });
        };

        let Some(document) = &state.document else {
            return Ok(Vec::new());
        };

        Ok(collect_embedded_regions(document)
            .iter()
            .map(embedded_protocol::to_dto)
            .collect())
    }

    async fn upsert_document(&self, uri: Url, text: String) {
        if self.is_registry_document(&uri).await {
            self.publish_registry_text(uri, &text).await;
            return;
        }

        let parsed = decode(&text);
        let mut diagnostics = match &parsed {
            Ok(document) => {
                let mut diags = analyze_document_name_kinds(document, &text);
                diags.extend(analyze_embedded_values(document, &text));
                diags.iter().map(convert::diagnostic).collect::<Vec<_>>()
            }
            Err(error) => error.diagnostics.iter().map(convert::diagnostic).collect(),
        };

        // Keep parse errors ahead of semantic ones when both somehow appear.
        diagnostics.sort_by_key(|diagnostic| {
            (
                diagnostic.range.start.line,
                diagnostic.range.start.character,
            )
        });

        {
            let mut documents = self.documents.write().await;
            documents.insert(
                uri.clone(),
                DocumentState {
                    text,
                    document: parsed.ok(),
                },
            );
        }

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn is_registry_document(&self, uri: &Url) -> bool {
        let path = uri.to_file_path().ok();
        let registry = self.registry.read().await;

        if let (Some(configured), Some(path)) = (&registry.path, &path) {
            if paths_equal(configured, path) {
                return true;
            }
        }

        path.as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".registry.json"))
    }

    async fn publish_registry_text(&self, uri: Url, text: &str) {
        let diagnostics = match decode_name_registry(text) {
            Ok(registry) => {
                let mut state = self.registry.write().await;
                state.uri = Some(uri.clone());
                state.registry = Some(registry);
                Vec::new()
            }
            Err(error) => {
                let mut state = self.registry.write().await;
                state.uri = Some(uri.clone());
                state.registry = None;
                error.diagnostics.iter().map(convert::diagnostic).collect()
            }
        };

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn set_registry_path(&self, path: Option<PathBuf>) {
        let previous_uri = {
            let mut state = self.registry.write().await;
            if state.path == path {
                return;
            }
            state.path = path;
            state.registry = None;
            state.uri.take()
        };

        if let Some(previous) = previous_uri {
            self.client
                .publish_diagnostics(previous, Vec::new(), None)
                .await;
        }

        self.reload_registry_from_disk().await;
    }

    async fn reload_registry_from_disk(&self) {
        let path = {
            let state = self.registry.read().await;
            state.path.clone()
        };

        let Some(path) = path else {
            return;
        };

        let uri = match Url::from_file_path(&path) {
            Ok(uri) => uri,
            Err(()) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("Invalid rainbow.registryPath: {}", path.display()),
                    )
                    .await;
                return;
            }
        };

        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("Loaded Rainbow name registry from {}", path.display()),
                    )
                    .await;
                self.publish_registry_text(uri, &text).await;
            }
            Err(error) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!(
                            "Could not read rainbow.registryPath ({}): {error}",
                            path.display()
                        ),
                    )
                    .await;
            }
        }
    }

    async fn apply_registry_path_setting(&self, value: &serde_json::Value) {
        let path = value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        self.set_registry_path(path).await;
    }

    fn registry_path_from_init(params: &InitializeParams) -> Option<PathBuf> {
        let options = params.initialization_options.as_ref()?;
        let path = options.get("registryPath")?.as_str()?.trim();
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    }

    fn registry_path_from_settings(settings: &serde_json::Value) -> Option<PathBuf> {
        let path = settings
            .pointer("/rainbow/registryPath")
            .or_else(|| settings.get("registryPath"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        Some(PathBuf::from(path))
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for RainbowLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(path) = Self::registry_path_from_init(&params) {
            let mut state = self.registry.write().await;
            state.path = Some(path);
        }

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "rainbow-lsp".to_string(),
                version: Some(RAINBOW_PARSER_VERSION.to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec!["@".to_string(), " ".to_string()]),
                    ..CompletionOptions::default()
                }),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("rainbow-lsp {RAINBOW_PARSER_VERSION} ready"),
            )
            .await;

        if let Ok(configs) = self
            .client
            .configuration(vec![ConfigurationItem {
                scope_uri: None,
                section: Some("rainbow.registryPath".to_string()),
            }])
            .await
        {
            if let Some(value) = configs.first() {
                self.apply_registry_path_setting(value).await;
            }
        }

        self.reload_registry_from_disk().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        if let Some(path) = Self::registry_path_from_settings(&params.settings) {
            self.set_registry_path(Some(path)).await;
        } else if params.settings.pointer("/rainbow/registryPath").is_some()
            || params.settings.get("registryPath").is_some()
        {
            self.set_registry_path(None).await;
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.upsert_document(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.upsert_document(params.text_document.uri, change.text)
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);

        if self.is_registry_document(&params.text_document.uri).await {
            // Keep disk-backed diagnostics for the configured registry path.
            self.reload_registry_from_disk().await;
            return;
        }

        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        if self.is_registry_document(&params.text_document.uri).await {
            return Ok(None);
        }

        let documents = self.documents.read().await;
        let Some(state) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };

        let Ok(document) = decode(&state.text) else {
            return Ok(None);
        };

        let formatted = encode(&document);
        if formatted == state.text {
            return Ok(Some(Vec::new()));
        }

        let end_line = state.text.lines().count().saturating_sub(1) as u32;
        let end_character = state
            .text
            .lines()
            .last()
            .map(|line| line.chars().count() as u32)
            .unwrap_or(0);

        Ok(Some(vec![TextEdit {
            range: Range::new(Position::new(0, 0), Position::new(end_line, end_character)),
            new_text: formatted,
        }]))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let documents = self.documents.read().await;
        let Some(state) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(document) = &state.document else {
            return Ok(None);
        };

        Ok(Some(DocumentSymbolResponse::Nested(
            convert::document_symbols(document),
        )))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let documents = self.documents.read().await;
        let Some(state) = documents.get(&params.text_document_position_params.text_document.uri)
        else {
            return Ok(None);
        };
        let Some(document) = &state.document else {
            return Ok(None);
        };

        let Some(markdown) = convert::hover_at(
            &state.text,
            document,
            params.text_document_position_params.position,
        ) else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let documents = self.documents.read().await;
        let Some(state) = documents.get(&params.text_document_position.text_document.uri) else {
            return Ok(None);
        };

        let line = state
            .text
            .lines()
            .nth(params.text_document_position.position.line as usize)
            .unwrap_or("");
        let prefix = &line[..params
            .text_document_position
            .position
            .character
            .min(line.len() as u32) as usize];

        let mut items = Vec::new();

        if prefix.trim().is_empty()
            || "use".starts_with(prefix.trim())
            || prefix.trim_start().starts_with('u')
        {
            items.push(CompletionItem {
                label: "use".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Version pin".to_string()),
                insert_text: Some("use ${1:Name}@${2:1.0.0}".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                documentation: Some(Documentation::String(
                    "Pin a plugin/event version: use Name@MAJOR.MINOR.PATCH".to_string(),
                )),
                ..CompletionItem::default()
            });
        }

        // Enum / identifier values must be written as `.name`.
        let trimmed_prefix = prefix.trim_end();
        if trimmed_prefix.ends_with(':')
            || trimmed_prefix.ends_with(": ")
            || trimmed_prefix.ends_with('.')
        {
            items.push(CompletionItem {
                label: ".primary".to_string(),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                detail: Some("Enum identifier value".to_string()),
                insert_text: Some(if trimmed_prefix.ends_with('.') {
                    "primary".to_string()
                } else {
                    ".${1:name}".to_string()
                }),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                documentation: Some(Documentation::String(
                    "Enum values require a leading '.' (e.g. .primary).".to_string(),
                )),
                ..CompletionItem::default()
            });

            for (label, snippet, detail) in [
                (
                    "@JSON",
                    "@JSON(${1:{\"key\": \"value\"}})",
                    "Embedded JSON value",
                ),
                ("@YAML", "@YAML(${1:key: value})", "Embedded YAML value"),
                ("@XML", "@XML(${1:<root/>})", "Embedded XML value"),
                ("@HTML", "@HTML(${1:<div></div>})", "Embedded HTML value"),
                (
                    "@MARKDOWN",
                    "@MARKDOWN(${1:# Title})",
                    "Embedded Markdown value",
                ),
            ] {
                items.push(CompletionItem {
                    label: label.to_string(),
                    kind: Some(CompletionItemKind::SNIPPET),
                    detail: Some(detail.to_string()),
                    insert_text: Some(snippet.to_string()),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    documentation: Some(Documentation::String(
                        "Embedded language payload: @LANG(...). Use @LANG(#{Name}) for a whole-blob template hole.".to_string(),
                    )),
                    ..CompletionItem::default()
                });
            }
        }

        {
            let registry = self.registry.read().await;
            if let Some(registry) = &registry.registry {
                for name in &registry.plugins {
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some("plugin (registry)".to_string()),
                        ..CompletionItem::default()
                    });
                }
                for name in &registry.events {
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: Some(CompletionItemKind::EVENT),
                        detail: Some("event (registry)".to_string()),
                        ..CompletionItem::default()
                    });
                }
            }
        }

        if let Some(document) = &state.document {
            for declaration in &document.uses {
                items.push(CompletionItem {
                    label: declaration.name.render(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(match &declaration.version {
                        Some(version) => format!("pinned @{version}"),
                        None => "template use (latest)".to_string(),
                    }),
                    ..CompletionItem::default()
                });
            }

            items.push(CompletionItem {
                label: "#{Name}".to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                detail: Some("Template placeholder".to_string()),
                insert_text: Some("#{${1:Name}}".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                documentation: Some(Documentation::String(
                    "Backend substitution hole. Same name replaces every occurrence.".to_string(),
                )),
                ..CompletionItem::default()
            });

            fn collect_names(node: &rainbow_parser::RainbowNode, items: &mut Vec<CompletionItem>) {
                items.push(CompletionItem {
                    label: node.name.render(),
                    kind: Some(CompletionItemKind::STRUCT),
                    detail: Some("Rainbow node".to_string()),
                    ..CompletionItem::default()
                });
                for child in node.children() {
                    collect_names(child, items);
                }
            }

            for node in &document.nodes {
                collect_names(node, &mut items);
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let documents = self.documents.read().await;
        let Some(state) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };

        let mut ranges = Vec::new();
        let mut stack = Vec::new();

        for (line_index, line) in state.text.lines().enumerate() {
            let open = line.matches('{').count();
            let close = line.matches('}').count();

            for _ in 0..open {
                stack.push(line_index as u32);
            }
            for _ in 0..close {
                if let Some(start_line) = stack.pop() {
                    let end_line = line_index as u32;
                    if end_line > start_line {
                        ranges.push(FoldingRange {
                            start_line,
                            end_line,
                            kind: Some(FoldingRangeKind::Region),
                            ..FoldingRange::default()
                        });
                    }
                }
            }
        }

        Ok(Some(ranges))
    }
}
