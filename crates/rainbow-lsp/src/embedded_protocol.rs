//! Custom LSP protocol for embedded `@LANG(...)` regions (IDE language services).

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{Range, TextDocumentIdentifier, Url};

use crate::convert;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedRegionsParams {
    pub text_document: TextDocumentIdentifier,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedRegionDto {
    pub language: String,
    pub language_id: String,
    pub body: String,
    pub body_range: Range,
    pub is_placeholder: bool,
}

pub fn language_id(language: rainbow_parser::EmbeddedLanguage) -> &'static str {
    match language {
        rainbow_parser::EmbeddedLanguage::Json => "json",
        rainbow_parser::EmbeddedLanguage::Yaml => "yaml",
        rainbow_parser::EmbeddedLanguage::Xml => "xml",
        rainbow_parser::EmbeddedLanguage::Html => "html",
        rainbow_parser::EmbeddedLanguage::Markdown => "markdown",
    }
}

pub fn to_dto(region: &rainbow_parser::EmbeddedRegion) -> EmbeddedRegionDto {
    EmbeddedRegionDto {
        language: region.language.as_str().to_string(),
        language_id: language_id(region.language).to_string(),
        body: region.body.clone(),
        body_range: convert::range_from_locations(region.body_range.start, region.body_range.end),
        is_placeholder: region.is_placeholder,
    }
}

pub fn document_uri(params: &EmbeddedRegionsParams) -> &Url {
    &params.text_document.uri
}
