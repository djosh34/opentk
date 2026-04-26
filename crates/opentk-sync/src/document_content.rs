use std::{fmt::Write as _, io::Read};

use chrono::{DateTime, Utc};
use quick_xml::{events::Event, Reader};
use reqwest::Url;
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;
use zip::ZipArchive;

use crate::document_asset::{DocumentAssetFetchReport, DocumentAssetKind, RetrievalStatus};

#[derive(Clone, Debug)]
pub struct DocumentExtractionFixture {
    pub source_hash: String,
    pub expected_text: Option<String>,
    pub expected_html: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DocumentContentExtractor {
    fixtures: Vec<DocumentExtractionFixture>,
}

impl DocumentContentExtractor {
    #[must_use]
    pub fn new(fixtures: Vec<DocumentExtractionFixture>) -> Self {
        Self { fixtures }
    }

    #[must_use]
    pub fn extract(
        &self,
        input: DocumentContentExtractionInput,
    ) -> DocumentContentExtractionReport {
        let raw = match extract_raw_content(&input) {
            Ok(raw) => raw,
            Err(error) => {
                return failed_report(input, error.status, error.validation, error.message)
            }
        };

        let output_hash = raw.output_bytes().map(sha256_hex);
        let mut report = DocumentContentExtractionReport {
            document_source_category: input.document_source_category,
            document_source_id: input.document_source_id,
            document_asset_id: input.document_asset_id,
            selected_source_url: input.selected_source_url,
            selected_source_content_type: input.selected_source_content_type,
            selected_source_content_length: input.selected_source_content_length,
            official_source: input.official_source,
            source_rank: input.source_rank,
            extraction_status: raw.status,
            validation_status: ValidationStatus::Unverified,
            extraction_error: None,
            extraction_tool: raw.tool,
            extraction_tool_version: raw.tool_version,
            source_hash: input.source_hash,
            output_hash,
            extracted_text: raw.text,
            extracted_html: raw.html,
            extracted_at: input.extracted_at,
        };

        apply_fixture_validation(&mut report, &self.fixtures);
        report
    }
}

#[derive(Clone, Debug)]
pub struct DocumentContentExtractionInput {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub document_asset_id: Option<i64>,
    pub selected_source_url: Url,
    pub selected_source_content_type: Option<String>,
    pub selected_source_content_length: Option<u64>,
    pub selected_source_kind: DocumentAssetKind,
    pub official_source: bool,
    pub source_rank: i32,
    pub source_hash: String,
    pub source_body: Vec<u8>,
    pub extracted_at: DateTime<Utc>,
}

impl DocumentContentExtractionInput {
    /// Build extraction input from a fetched asset report.
    ///
    /// # Errors
    ///
    /// Returns an error when the report was not fetched or omits selected
    /// source metadata, body bytes, or source hash.
    pub fn from_fetch_report(
        document_asset_id: i64,
        report: &DocumentAssetFetchReport,
    ) -> Result<Self, DocumentContentExtractionInputError> {
        if report.retrieval_status != RetrievalStatus::Fetched {
            return Err(DocumentContentExtractionInputError::NotFetched);
        }
        let selected = report.selected_source.as_ref().ok_or(
            DocumentContentExtractionInputError::MissingField("selected_source"),
        )?;
        let source_body = report.selected_body.clone().ok_or(
            DocumentContentExtractionInputError::MissingField("selected_body"),
        )?;
        let source_hash =
            report
                .source_hash
                .clone()
                .ok_or(DocumentContentExtractionInputError::MissingField(
                    "source_hash",
                ))?;

        Ok(Self {
            document_source_category: report.document_source_category.clone(),
            document_source_id: report.document_source_id,
            document_asset_id: Some(document_asset_id),
            selected_source_url: selected.url.clone(),
            selected_source_content_type: selected.content_type.clone(),
            selected_source_content_length: selected.content_length,
            selected_source_kind: selected.kind.clone(),
            official_source: selected.official_source,
            source_rank: selected.source_rank,
            source_hash,
            source_body,
            extracted_at: report.retrieved_at,
        })
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DocumentContentExtractionInputError {
    #[error("document asset report was not fetched")]
    NotFetched,
    #[error("document asset report is missing {0}")]
    MissingField(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentContentExtractionReport {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub document_asset_id: Option<i64>,
    pub selected_source_url: Url,
    pub selected_source_content_type: Option<String>,
    pub selected_source_content_length: Option<u64>,
    pub official_source: bool,
    pub source_rank: i32,
    pub extraction_status: ExtractionStatus,
    pub validation_status: ValidationStatus,
    pub extraction_error: Option<String>,
    pub extraction_tool: String,
    pub extraction_tool_version: String,
    pub source_hash: String,
    pub output_hash: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub extracted_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtractionStatus {
    Extracted,
    Empty,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationStatus {
    Unverified,
    Valid,
    Invalid,
}

struct RawExtractedContent {
    status: ExtractionStatus,
    tool: String,
    tool_version: String,
    text: Option<String>,
    html: Option<String>,
}

impl RawExtractedContent {
    fn output_bytes(&self) -> Option<Vec<u8>> {
        let mut output = Vec::new();
        if let Some(text) = &self.text {
            output.extend_from_slice(text.as_bytes());
        }
        if let Some(html) = &self.html {
            if !output.is_empty() {
                output.push(b'\n');
            }
            output.extend_from_slice(html.as_bytes());
        }
        (!output.is_empty()).then_some(output)
    }
}

struct ExtractionFailure {
    status: ExtractionStatus,
    validation: ValidationStatus,
    message: String,
}

fn extract_raw_content(
    input: &DocumentContentExtractionInput,
) -> Result<RawExtractedContent, ExtractionFailure> {
    match input.selected_source_kind {
        DocumentAssetKind::OfficialText | DocumentAssetKind::OfficialTranscript => {
            let text = decode_utf8(&input.source_body, "official text")?;
            let text = normalize_text(&text);
            Ok(extracted_or_empty("official-source", "1", Some(text), None))
        }
        DocumentAssetKind::OfficialHtml => extract_html(input, "official-html", "1"),
        DocumentAssetKind::Docx => extract_docx(input),
        DocumentAssetKind::Pdf => extract_pdf(input),
    }
}

fn extract_html(
    input: &DocumentContentExtractionInput,
    tool: &str,
    version: &str,
) -> Result<RawExtractedContent, ExtractionFailure> {
    let html = decode_utf8(&input.source_body, "HTML")?;
    let text = extract_visible_html_text(&html);
    Ok(extracted_or_empty(tool, version, Some(text), Some(html)))
}

fn extract_docx(
    input: &DocumentContentExtractionInput,
) -> Result<RawExtractedContent, ExtractionFailure> {
    let cursor = std::io::Cursor::new(&input.source_body);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|source| invalid_failure(format!("DOCX ZIP parse failed: {source}")))?;
    let mut document = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|source| invalid_failure(format!("DOCX missing word/document.xml: {source}")))?
        .read_to_string(&mut document)
        .map_err(|source| invalid_failure(format!("DOCX document.xml is not UTF-8: {source}")))?;
    let text = extract_docx_text(&document)?;
    Ok(extracted_or_empty(
        "docx-zip-quick-xml",
        env!("CARGO_PKG_VERSION"),
        Some(text),
        None,
    ))
}

fn extract_pdf(
    input: &DocumentContentExtractionInput,
) -> Result<RawExtractedContent, ExtractionFailure> {
    let text = pdf_extract::extract_text_from_mem(&input.source_body)
        .map_err(|source| invalid_failure(format!("PDF extraction failed: {source}")))?;
    Ok(extracted_or_empty(
        "pdf-extract",
        "0.10.0",
        Some(normalize_text(&text)),
        None,
    ))
}

fn extracted_or_empty(
    tool: &str,
    version: &str,
    text: Option<String>,
    html: Option<String>,
) -> RawExtractedContent {
    let text = text.filter(|value| !value.is_empty());
    let html = html.filter(|value| !value.is_empty());
    let status = if text.is_some() || html.is_some() {
        ExtractionStatus::Extracted
    } else {
        ExtractionStatus::Empty
    };
    RawExtractedContent {
        status,
        tool: tool.to_owned(),
        tool_version: version.to_owned(),
        text,
        html,
    }
}

fn decode_utf8(bytes: &[u8], context: &'static str) -> Result<String, ExtractionFailure> {
    String::from_utf8(bytes.to_vec())
        .map_err(|source| invalid_failure(format!("{context} is not UTF-8: {source}")))
}

fn extract_visible_html_text(html: &str) -> String {
    let document = Html::parse_document(html);
    let block_selector = Selector::parse(
        "body h1, body h2, body h3, body h4, body h5, body h6, body p, body li, body td, body th",
    )
    .expect("static selector parses");
    let blocks: Vec<_> = document
        .select(&block_selector)
        .map(|element| normalize_inline_text(element.text()))
        .filter(|text| !text.is_empty())
        .collect();
    if blocks.is_empty() {
        let body_selector = Selector::parse("body").expect("static selector parses");
        return normalize_text(
            &document
                .select(&body_selector)
                .flat_map(|element| element.text())
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    normalize_text(&blocks.join("\n\n"))
}

fn extract_docx_text(document_xml: &str) -> Result<String, ExtractionFailure> {
    let mut reader = Reader::from_str(document_xml);
    reader.config_mut().trim_text(true);
    let mut output = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = element.name();
                if is_word_name(name.as_ref(), b"t") {
                    in_text = true;
                } else if is_word_name(name.as_ref(), b"p") {
                    push_paragraph_break(&mut output);
                } else if is_word_name(name.as_ref(), b"tab") || is_word_name(name.as_ref(), b"tc")
                {
                    push_space(&mut output);
                } else if is_word_name(name.as_ref(), b"br") {
                    output.push('\n');
                }
            }
            Ok(Event::Empty(element)) => {
                let name = element.name();
                if is_word_name(name.as_ref(), b"tab") || is_word_name(name.as_ref(), b"tc") {
                    push_space(&mut output);
                } else if is_word_name(name.as_ref(), b"br") {
                    output.push('\n');
                }
            }
            Ok(Event::End(element)) => {
                let name = element.name();
                if is_word_name(name.as_ref(), b"t") {
                    in_text = false;
                } else if is_word_name(name.as_ref(), b"p") {
                    push_paragraph_break(&mut output);
                }
            }
            Ok(Event::Text(text)) if in_text => {
                let value = text.unescape().map_err(|source| {
                    invalid_failure(format!("DOCX text unescape failed: {source}"))
                })?;
                output.push_str(&value);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(source) => return Err(invalid_failure(format!("DOCX XML parse failed: {source}"))),
        }
    }
    Ok(normalize_text(&output))
}

fn is_word_name(name: &[u8], local: &[u8]) -> bool {
    name == local || name.strip_prefix(b"w:") == Some(local)
}

fn push_space(output: &mut String) {
    if !output.ends_with([' ', '\n']) {
        output.push(' ');
    }
}

fn push_paragraph_break(output: &mut String) {
    let trimmed = output.trim_end_matches([' ', '\t']);
    output.truncate(trimmed.len());
    if !output.is_empty() && !output.ends_with("\n\n") {
        if output.ends_with('\n') {
            output.push('\n');
        } else {
            output.push_str("\n\n");
        }
    }
}

fn normalize_inline_text<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_text(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = Vec::new();
    let mut blank_count = 0usize;
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                lines.push(String::new());
            }
        } else {
            blank_count = 0;
            lines.push(line.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    lines.join("\n").trim().to_owned()
}

fn apply_fixture_validation(
    report: &mut DocumentContentExtractionReport,
    fixtures: &[DocumentExtractionFixture],
) {
    let Some(fixture) = fixtures
        .iter()
        .find(|fixture| fixture.source_hash == report.source_hash)
    else {
        return;
    };

    let expected_text = fixture.expected_text.as_deref().map(normalize_text);
    let expected_html = fixture.expected_html.clone();
    let text_matches = expected_text.as_deref() == report.extracted_text.as_deref();
    let html_matches = expected_html.as_deref() == report.extracted_html.as_deref();

    if text_matches && html_matches {
        report.validation_status = ValidationStatus::Valid;
        return;
    }

    report.extraction_status = ExtractionStatus::Failed;
    report.validation_status = ValidationStatus::Invalid;
    report.extraction_error = Some("fixture mismatch for extracted document content".to_owned());
}

fn failed_report(
    input: DocumentContentExtractionInput,
    status: ExtractionStatus,
    validation_status: ValidationStatus,
    error: String,
) -> DocumentContentExtractionReport {
    DocumentContentExtractionReport {
        document_source_category: input.document_source_category,
        document_source_id: input.document_source_id,
        document_asset_id: input.document_asset_id,
        selected_source_url: input.selected_source_url,
        selected_source_content_type: input.selected_source_content_type,
        selected_source_content_length: input.selected_source_content_length,
        official_source: input.official_source,
        source_rank: input.source_rank,
        extraction_status: status,
        validation_status,
        extraction_error: Some(error),
        extraction_tool: extraction_tool_for_kind(&input.selected_source_kind).to_owned(),
        extraction_tool_version: extraction_tool_version_for_kind(&input.selected_source_kind)
            .to_owned(),
        source_hash: input.source_hash,
        output_hash: None,
        extracted_text: None,
        extracted_html: None,
        extracted_at: input.extracted_at,
    }
}

fn extraction_tool_for_kind(kind: &DocumentAssetKind) -> &'static str {
    match kind {
        DocumentAssetKind::OfficialText | DocumentAssetKind::OfficialTranscript => {
            "official-source"
        }
        DocumentAssetKind::OfficialHtml => "official-html",
        DocumentAssetKind::Docx => "docx-zip-quick-xml",
        DocumentAssetKind::Pdf => "pdf-extract",
    }
}

fn extraction_tool_version_for_kind(kind: &DocumentAssetKind) -> &'static str {
    match kind {
        DocumentAssetKind::OfficialText
        | DocumentAssetKind::OfficialTranscript
        | DocumentAssetKind::OfficialHtml => "1",
        DocumentAssetKind::Docx => env!("CARGO_PKG_VERSION"),
        DocumentAssetKind::Pdf => "0.10.0",
    }
}

fn invalid_failure(message: String) -> ExtractionFailure {
    ExtractionFailure {
        status: ExtractionStatus::Failed,
        validation: ValidationStatus::Invalid,
        message,
    }
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    let digest = Sha256::digest(bytes.as_ref());
    digest.iter().fold(String::new(), |mut output, byte| {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        output
    })
}
