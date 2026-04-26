use opentk_sync::document_asset::{DocumentAssetFetchReport, DocumentAssetKind, RetrievalStatus};
use sqlx::{PgPool, Postgres};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentAssetWriteOutcome {
    pub assets_recorded: usize,
    pub contents_recorded: usize,
    pub failures_recorded: usize,
}

#[derive(Debug, Error)]
pub enum DocumentAssetWriteError {
    #[error("content length {value} cannot be stored as PostgreSQL bigint")]
    ContentLengthOutOfRange { value: u64 },
    #[error("fetched official content report is missing {field}")]
    InvalidContentReport { field: &'static str },
    #[error("database write failed: {0}")]
    Sql(#[from] sqlx::Error),
}

/// Record document asset retrieval reports in `PostgreSQL`.
///
/// # Errors
///
/// Returns an error when `PostgreSQL` rejects any asset or content write, a
/// content length cannot be represented as `bigint`, or a fetched official
/// content report omits the selected source, selected body, or content hash.
pub async fn record_document_asset_fetches(
    pool: &PgPool,
    reports: &[DocumentAssetFetchReport],
) -> Result<DocumentAssetWriteOutcome, DocumentAssetWriteError> {
    let mut tx = pool.begin().await?;
    let mut outcome = DocumentAssetWriteOutcome {
        assets_recorded: 0,
        contents_recorded: 0,
        failures_recorded: 0,
    };

    for report in reports {
        let document_asset_id = upsert_asset(&mut tx, report).await?;
        outcome.assets_recorded += 1;
        if report.retrieval_status != RetrievalStatus::Fetched {
            outcome.failures_recorded += 1;
        }
        if should_record_content(report) {
            insert_content(&mut tx, document_asset_id, report).await?;
            outcome.contents_recorded += 1;
        }
    }

    tx.commit().await?;
    Ok(outcome)
}

async fn upsert_asset(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    report: &DocumentAssetFetchReport,
) -> Result<i64, DocumentAssetWriteError> {
    let id = sqlx::query_scalar(
        r"
        INSERT INTO document_asset (
            document_source_category,
            document_source_id,
            asset_url,
            upstream_url,
            upstream_content_type,
            upstream_content_length,
            upstream_last_modified_at,
            retrieval_status,
            retrieval_error,
            retrieved_at,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), now())
        ON CONFLICT (document_source_category, document_source_id, asset_url) DO UPDATE
        SET upstream_url = EXCLUDED.upstream_url,
            upstream_content_type = EXCLUDED.upstream_content_type,
            upstream_content_length = EXCLUDED.upstream_content_length,
            upstream_last_modified_at = EXCLUDED.upstream_last_modified_at,
            retrieval_status = EXCLUDED.retrieval_status,
            retrieval_error = EXCLUDED.retrieval_error,
            retrieved_at = EXCLUDED.retrieved_at,
            updated_at = EXCLUDED.updated_at
        RETURNING id
        ",
    )
    .bind(&report.document_source_category)
    .bind(report.document_source_id)
    .bind(report.asset_url.as_str())
    .bind(report.upstream_url.as_str())
    .bind(&report.upstream_content_type)
    .bind(optional_i64_from_u64(report.upstream_content_length)?)
    .bind(report.upstream_last_modified_at)
    .bind(retrieval_status_sql(&report.retrieval_status))
    .bind(&report.retrieval_error)
    .bind(report.retrieved_at)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

async fn insert_content(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    document_asset_id: i64,
    report: &DocumentAssetFetchReport,
) -> Result<(), DocumentAssetWriteError> {
    let selected =
        report
            .selected_source
            .as_ref()
            .ok_or(DocumentAssetWriteError::InvalidContentReport {
                field: "selected_source",
            })?;
    let body =
        report
            .selected_body
            .as_deref()
            .ok_or(DocumentAssetWriteError::InvalidContentReport {
                field: "selected_body",
            })?;
    let body = String::from_utf8_lossy(body);
    let (extracted_text, extracted_html) = match selected.kind {
        DocumentAssetKind::OfficialText | DocumentAssetKind::OfficialTranscript => {
            (Some(body.as_ref()), None)
        }
        DocumentAssetKind::OfficialHtml => (None, Some(body.as_ref())),
        DocumentAssetKind::Pdf | DocumentAssetKind::Docx => (None, None),
    };

    sqlx::query(
        r"
        INSERT INTO document_content (
            document_asset_id,
            document_source_category,
            document_source_id,
            selected_source_url,
            selected_source_content_type,
            selected_source_content_length,
            official_source,
            source_rank,
            extraction_status,
            validation_status,
            extraction_tool,
            extraction_tool_version,
            content_hash,
            extracted_text,
            extracted_html,
            extracted_at,
            created_at,
            updated_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            'extracted', 'unverified', 'official-source', '1',
            $9, $10, $11, $12, now(), now()
        )
        ON CONFLICT (document_asset_id, content_hash) DO UPDATE
        SET selected_source_url = EXCLUDED.selected_source_url,
            selected_source_content_type = EXCLUDED.selected_source_content_type,
            selected_source_content_length = EXCLUDED.selected_source_content_length,
            official_source = EXCLUDED.official_source,
            source_rank = EXCLUDED.source_rank,
            extraction_status = EXCLUDED.extraction_status,
            validation_status = EXCLUDED.validation_status,
            extraction_tool = EXCLUDED.extraction_tool,
            extraction_tool_version = EXCLUDED.extraction_tool_version,
            extracted_text = EXCLUDED.extracted_text,
            extracted_html = EXCLUDED.extracted_html,
            extracted_at = EXCLUDED.extracted_at,
            updated_at = EXCLUDED.updated_at
        ",
    )
    .bind(document_asset_id)
    .bind(&report.document_source_category)
    .bind(report.document_source_id)
    .bind(selected.url.as_str())
    .bind(&selected.content_type)
    .bind(optional_i64_from_u64(selected.content_length)?)
    .bind(selected.official_source)
    .bind(selected.source_rank)
    .bind(
        report
            .content_hash
            .as_deref()
            .ok_or(DocumentAssetWriteError::InvalidContentReport {
                field: "content_hash",
            })?,
    )
    .bind(extracted_text)
    .bind(extracted_html)
    .bind(report.retrieved_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn should_record_content(report: &DocumentAssetFetchReport) -> bool {
    if report.retrieval_status != RetrievalStatus::Fetched {
        return false;
    }
    let Some(selected) = &report.selected_source else {
        return false;
    };
    selected.official_source
        && matches!(
            selected.kind,
            DocumentAssetKind::OfficialText
                | DocumentAssetKind::OfficialHtml
                | DocumentAssetKind::OfficialTranscript
        )
        && report.selected_body.is_some()
        && report.content_hash.is_some()
}

const fn retrieval_status_sql(status: &RetrievalStatus) -> &'static str {
    match status {
        RetrievalStatus::Fetched => "fetched",
        RetrievalStatus::NotFound => "not_found",
        RetrievalStatus::UnsupportedContentType => "unsupported_content_type",
        RetrievalStatus::Failed => "failed",
    }
}

fn optional_i64_from_u64(value: Option<u64>) -> Result<Option<i64>, DocumentAssetWriteError> {
    value
        .map(|value| {
            i64::try_from(value)
                .map_err(|_| DocumentAssetWriteError::ContentLengthOutOfRange { value })
        })
        .transpose()
}
