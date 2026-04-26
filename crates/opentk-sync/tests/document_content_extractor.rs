use chrono::{DateTime, Utc};
use opentk_sync::{
    document_asset::{
        DocumentAssetFetchReport, DocumentAssetKind, DocumentSelectedSource, RetrievalStatus,
    },
    document_content::{
        DocumentContentExtractionInput, DocumentContentExtractor, DocumentExtractionFixture,
        ExtractionStatus, ValidationStatus,
    },
};
use reqwest::Url;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const HTML_FIXTURE: &[u8] = include_bytes!("fixtures/document_content/fixture.html");
const HTML_EXPECTED_HTML: &str =
    include_str!("fixtures/document_content/fixture.html.expected.html");
const HTML_EXPECTED_TEXT: &str =
    include_str!("fixtures/document_content/fixture.html.expected.txt");
const DOCX_FIXTURE: &[u8] = include_bytes!("fixtures/document_content/fixture.docx");
const DOCX_EXPECTED_TEXT: &str =
    include_str!("fixtures/document_content/fixture.docx.expected.txt");
const PDF_FIXTURE: &[u8] = include_bytes!("fixtures/document_content/fixture.pdf");
const PDF_EXPECTED_TEXT: &str = include_str!("fixtures/document_content/fixture.pdf.expected.txt");

#[test]
fn official_text_is_selected_normalized_hashed_and_validated() {
    let source = b" Official transcript text.\r\n\r\n\r\nSecond line. ";
    let expected = "Official transcript text.\n\nSecond line.";
    let extractor = DocumentContentExtractor::new(vec![fixture(source, Some(expected), None)]);

    let report = extractor.extract(input(
        DocumentAssetKind::OfficialText,
        true,
        "text/plain",
        source,
    ));

    assert_eq!(report.extraction_status, ExtractionStatus::Extracted);
    assert_eq!(report.validation_status, ValidationStatus::Valid);
    assert_eq!(report.extraction_error, None);
    assert_eq!(report.extraction_tool, "official-source");
    assert_eq!(report.extraction_tool_version, "1");
    assert_eq!(report.source_hash, sha256_hex(source));
    assert_eq!(
        report.output_hash.as_deref(),
        Some(sha256_hex(expected.as_bytes()).as_str())
    );
    assert_eq!(report.extracted_text.as_deref(), Some(expected));
    assert_eq!(report.extracted_html, None);
}

#[test]
fn html_fixture_stores_html_and_exact_visible_text() {
    let extractor = DocumentContentExtractor::new(vec![fixture(
        HTML_FIXTURE,
        Some(expected(HTML_EXPECTED_TEXT)),
        Some(HTML_EXPECTED_HTML),
    )]);

    let first = extractor.extract(input(
        DocumentAssetKind::OfficialHtml,
        true,
        "text/html",
        HTML_FIXTURE,
    ));
    let second = extractor.extract(input(
        DocumentAssetKind::OfficialHtml,
        true,
        "text/html",
        HTML_FIXTURE,
    ));

    assert_eq!(first.extraction_status, ExtractionStatus::Extracted);
    assert_eq!(first.validation_status, ValidationStatus::Valid);
    assert_eq!(first.extracted_html.as_deref(), Some(HTML_EXPECTED_HTML));
    assert_eq!(
        first.extracted_text.as_deref(),
        Some(expected(HTML_EXPECTED_TEXT))
    );
    assert_eq!(first.output_hash, second.output_hash);
    assert_eq!(first.extracted_text, second.extracted_text);
}

#[test]
fn docx_fixture_extracts_exact_expected_text() {
    let extractor = DocumentContentExtractor::new(vec![fixture(
        DOCX_FIXTURE,
        Some(expected(DOCX_EXPECTED_TEXT)),
        None,
    )]);

    let report = extractor.extract(input(
        DocumentAssetKind::Docx,
        false,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        DOCX_FIXTURE,
    ));

    assert_eq!(
        report.extraction_status,
        ExtractionStatus::Extracted,
        "{report:?}"
    );
    assert_eq!(report.validation_status, ValidationStatus::Valid);
    assert_eq!(report.extraction_tool, "docx-zip-quick-xml");
    assert_eq!(
        report.extracted_text.as_deref(),
        Some(expected(DOCX_EXPECTED_TEXT))
    );
}

#[test]
fn pdf_fixture_extracts_exact_expected_text() {
    let extractor = DocumentContentExtractor::new(vec![fixture(
        PDF_FIXTURE,
        Some(expected(PDF_EXPECTED_TEXT)),
        None,
    )]);

    let report = extractor.extract(input(
        DocumentAssetKind::Pdf,
        false,
        "application/pdf",
        PDF_FIXTURE,
    ));

    assert_eq!(
        report.extraction_status,
        ExtractionStatus::Extracted,
        "{report:?}"
    );
    assert_eq!(report.validation_status, ValidationStatus::Valid);
    assert_eq!(report.extraction_tool, "pdf-extract");
    assert_eq!(
        report.extracted_text.as_deref(),
        Some(expected(PDF_EXPECTED_TEXT))
    );
}

#[test]
fn fixture_mismatch_is_an_explicit_invalid_failure() {
    let extractor =
        DocumentContentExtractor::new(vec![fixture(b"actual text", Some("different text"), None)]);

    let report = extractor.extract(input(
        DocumentAssetKind::OfficialText,
        true,
        "text/plain",
        b"actual text",
    ));

    assert_eq!(report.extraction_status, ExtractionStatus::Failed);
    assert_eq!(report.validation_status, ValidationStatus::Invalid);
    assert!(report
        .extraction_error
        .as_deref()
        .unwrap_or_default()
        .contains("fixture mismatch"));
    assert_eq!(report.source_hash, sha256_hex(b"actual text"));
    assert_eq!(report.extraction_tool, "official-source");
}

#[test]
fn fetched_report_conversion_prefers_selected_official_source() {
    let document_source_id = document_id();
    let selected_url = Url::parse("https://example.test/document.txt").expect("url");
    let report = DocumentAssetFetchReport {
        document_source_category: "Document".to_owned(),
        document_source_id,
        asset_url: Url::parse("https://example.test/document.pdf").expect("url"),
        upstream_url: Url::parse("https://example.test/document.pdf").expect("url"),
        upstream_content_type: Some("application/pdf".to_owned()),
        upstream_content_length: Some(99),
        upstream_last_modified_at: None,
        retrieval_status: RetrievalStatus::Fetched,
        retrieval_error: None,
        retrieved_at: timestamp(),
        source_hash: Some(sha256_hex(b"official text")),
        selected_source: Some(DocumentSelectedSource {
            url: selected_url.clone(),
            content_type: Some("text/plain".to_owned()),
            content_length: Some(13),
            kind: DocumentAssetKind::OfficialText,
            official_source: true,
            source_rank: 0,
        }),
        selected_body: Some(b"official text".to_vec()),
        discovered_sources: Vec::new(),
    };

    let input = DocumentContentExtractionInput::from_fetch_report(42, &report).expect("input");

    assert_eq!(input.document_asset_id, Some(42));
    assert_eq!(input.selected_source_url, selected_url);
    assert_eq!(input.selected_source_kind, DocumentAssetKind::OfficialText);
    assert!(input.official_source);
    assert_eq!(input.source_body, b"official text");
}

fn fixture(
    source: &[u8],
    expected_text: Option<&str>,
    expected_html: Option<&str>,
) -> DocumentExtractionFixture {
    DocumentExtractionFixture {
        source_hash: sha256_hex(source),
        expected_text: expected_text.map(str::to_owned),
        expected_html: expected_html.map(str::to_owned),
    }
}

fn expected(value: &str) -> &str {
    value.trim_end_matches('\n')
}

fn input(
    kind: DocumentAssetKind,
    official_source: bool,
    content_type: &str,
    body: &[u8],
) -> DocumentContentExtractionInput {
    DocumentContentExtractionInput {
        document_source_category: "Document".to_owned(),
        document_source_id: document_id(),
        document_asset_id: Some(42),
        selected_source_url: Url::parse("https://example.test/document").expect("url"),
        selected_source_content_type: Some(content_type.to_owned()),
        selected_source_content_length: Some(body.len() as u64),
        selected_source_kind: kind,
        official_source,
        source_rank: if official_source { 0 } else { 10 },
        source_hash: sha256_hex(body),
        source_body: body.to_vec(),
        extracted_at: timestamp(),
    }
}

fn document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("uuid")
}

fn timestamp() -> DateTime<Utc> {
    "2026-04-26T02:00:00Z".parse().expect("timestamp")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    digest.iter().fold(String::new(), |mut output, byte| {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        output
    })
}
