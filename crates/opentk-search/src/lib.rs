use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use uuid::Uuid;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SearchMappingError {
    #[error("search source record is missing required field {field}")]
    MissingRequiredField { field: &'static str },
    #[error("search source record field {field} has unsupported value type {value_type}")]
    UnsupportedFieldType {
        field: String,
        value_type: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchSourceRecord {
    pub metadata: SearchEntityMetadata,
    pub fields: Map<String, Value>,
    pub document_content: Option<SearchDocumentContent>,
    pub relations: Vec<SearchRelationLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntityMetadata {
    pub category: String,
    pub source_id: Uuid,
    pub latest_skiptoken: i64,
    pub deleted: bool,
    pub source_updated_at: DateTime<Utc>,
    pub atom_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDocumentContent {
    pub selected_source_url: String,
    pub selected_source_content_type: Option<String>,
    pub official_source: bool,
    pub extraction_status: String,
    pub validation_status: String,
    pub output_hash: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRelationLabel {
    pub relation_name: String,
    pub target_category: String,
    pub target_id: Uuid,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchIndexOperation {
    Upsert(Box<SearchIndexDocument>),
    Delete(SearchDocumentKey),
}

pub type SearchDocumentKey = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexDocument {
    pub key: SearchDocumentKey,
    pub source_category: String,
    pub source_id: Uuid,
    pub entity_kind: SearchEntityKind,
    pub title: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub date: Option<String>,
    pub document_number: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub metadata_text: Vec<String>,
    pub relation_labels: Vec<String>,
    pub filter_categories: Vec<String>,
    pub latest_skiptoken: i64,
    pub source_updated_at: DateTime<Utc>,
    pub atom_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexSchema {
    pub index_name: &'static str,
    pub primary_key: &'static str,
    pub searchable_attributes: Vec<&'static str>,
    pub displayed_attributes: Vec<&'static str>,
    pub filterable_attributes: Vec<&'static str>,
    pub sortable_attributes: Vec<&'static str>,
    pub ranking_rules: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchEntityKind {
    Document,
    Person,
    Activity,
    Dossier,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchApiResultShape {
    pub fields: Vec<&'static str>,
    pub snippet_fields: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchStorageSample {
    pub record_count: u64,
    pub extracted_text_bytes: u64,
    pub extracted_html_bytes: u64,
    pub relation_label_bytes: u64,
    pub engine_overhead_factor: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStorageEstimate {
    pub estimated_index_bytes: u64,
    pub bytes_per_record_from_task_1_fixture: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SearchStorageError {
    #[error("storage sample record count must be greater than zero")]
    EmptySample,
    #[error("storage overhead factor must be greater than zero")]
    InvalidOverheadFactor,
}

#[must_use]
pub fn meilisearch_schema() -> SearchIndexSchema {
    SearchIndexSchema {
        index_name: "opentk_entities",
        primary_key: "key",
        searchable_attributes: vec![
            "title",
            "summary",
            "document_number",
            "metadata_text",
            "relation_labels",
            "extracted_text",
            "extracted_html",
        ],
        displayed_attributes: vec![
            "key",
            "source_category",
            "source_id",
            "entity_kind",
            "title",
            "summary",
            "source_url",
            "date",
            "document_number",
            "metadata_text",
            "relation_labels",
            "latest_skiptoken",
            "source_updated_at",
            "_formatted",
        ],
        filterable_attributes: vec![
            "source_category",
            "entity_kind",
            "date",
            "filter_categories",
            "latest_skiptoken",
        ],
        sortable_attributes: vec!["date", "source_updated_at", "latest_skiptoken"],
        ranking_rules: vec![
            "words",
            "typo",
            "proximity",
            "attribute",
            "sort",
            "exactness",
        ],
    }
}

#[must_use]
pub fn api_result_shape() -> SearchApiResultShape {
    SearchApiResultShape {
        fields: vec![
            "key",
            "source_category",
            "source_id",
            "entity_kind",
            "title",
            "summary",
            "source_url",
            "date",
            "document_number",
            "snippets",
            "ranking_score",
        ],
        snippet_fields: vec![
            "title",
            "summary",
            "metadata_text",
            "extracted_text",
            "extracted_html",
        ],
    }
}

/// Estimates search index storage from measured source content bytes.
///
/// # Errors
///
/// Returns an error when the sample has no records or uses an invalid overhead factor.
pub fn storage_estimate(
    sample: SearchStorageSample,
) -> Result<SearchStorageEstimate, SearchStorageError> {
    if sample.record_count == 0 {
        return Err(SearchStorageError::EmptySample);
    }
    if sample.engine_overhead_factor == 0 {
        return Err(SearchStorageError::InvalidOverheadFactor);
    }

    let source_bytes =
        sample.extracted_text_bytes + sample.extracted_html_bytes + sample.relation_label_bytes;
    Ok(SearchStorageEstimate {
        estimated_index_bytes: source_bytes * sample.engine_overhead_factor,
        bytes_per_record_from_task_1_fixture: 716,
    })
}

/// Maps one source record from `PostgreSQL` into the operation to submit to the search index.
///
/// # Errors
///
/// Returns an error when a required identity/title/body field is absent or when
/// scalar metadata cannot be converted into stable searchable text.
pub fn map_record_to_operation(
    record: &SearchSourceRecord,
) -> Result<SearchIndexOperation, SearchMappingError> {
    let key = document_key(&record.metadata.category, record.metadata.source_id);
    if record.metadata.deleted {
        return Ok(SearchIndexOperation::Delete(key));
    }

    let entity_kind = entity_kind(&record.metadata.category);
    let title = title_for_record(record)?;
    let document_content = record.document_content.as_ref();
    let source_url = source_url(record, document_content);
    let document_number = optional_string(&record.fields, "document_nummer");
    let extracted_text =
        document_content.and_then(|content| present_string(content.extracted_text.as_deref()));
    let extracted_html =
        document_content.and_then(|content| present_string(content.extracted_html.as_deref()));

    if entity_kind == SearchEntityKind::Document {
        require_option(document_number.as_ref(), "document_nummer")?;
        require_field(&record.fields, "content_type")?;
        require_field(&record.fields, "content_length")?;
        require_option(extracted_text.as_ref(), "document_content.extracted_text")?;
        require_option(extracted_html.as_ref(), "document_content.extracted_html")?;
    }

    let metadata_text = metadata_text(&record.fields, document_content)?;
    let relation_labels = record
        .relations
        .iter()
        .map(|relation| {
            format!(
                "{} {} {}",
                relation.relation_name, relation.target_category, relation.label
            )
        })
        .collect::<Vec<_>>();
    let filter_categories = filter_categories(record);

    Ok(SearchIndexOperation::Upsert(Box::new(
        SearchIndexDocument {
            key,
            source_category: record.metadata.category.clone(),
            source_id: record.metadata.source_id,
            entity_kind,
            title,
            summary: optional_string(&record.fields, "samenvatting"),
            source_url,
            date: optional_string(&record.fields, "datum"),
            document_number,
            extracted_text,
            extracted_html,
            metadata_text,
            relation_labels,
            filter_categories,
            latest_skiptoken: record.metadata.latest_skiptoken,
            source_updated_at: record.metadata.source_updated_at,
            atom_updated_at: record.metadata.atom_updated_at,
        },
    )))
}

fn filter_categories(record: &SearchSourceRecord) -> Vec<String> {
    let target_categories = record
        .relations
        .iter()
        .filter_map(|relation| present_string(Some(relation.target_category.as_str())))
        .collect::<BTreeSet<_>>();
    let mut categories = vec![record.metadata.category.clone()];
    categories.extend(
        target_categories
            .into_iter()
            .filter(|category| category != &record.metadata.category),
    );
    categories
}

fn document_key(source_category: &str, source_id: Uuid) -> SearchDocumentKey {
    format!("{source_category}:{source_id}")
}

fn entity_kind(source_category: &str) -> SearchEntityKind {
    match source_category {
        "Document" => SearchEntityKind::Document,
        "Persoon" => SearchEntityKind::Person,
        "Activiteit" => SearchEntityKind::Activity,
        "Kamerstukdossier" | "Zaak" => SearchEntityKind::Dossier,
        _ => SearchEntityKind::Other,
    }
}

fn title_for_record(record: &SearchSourceRecord) -> Result<String, SearchMappingError> {
    for field in ["titel", "onderwerp", "nummer", "document_nummer"] {
        if let Some(value) = optional_string(&record.fields, field) {
            return Ok(value);
        }
    }

    if record.metadata.category == "Persoon" {
        if let Some(value) = person_display_name(&record.fields) {
            return Ok(value);
        }
    }

    record
        .relations
        .iter()
        .find_map(|relation| present_string(Some(relation.label.as_str())))
        .or_else(|| {
            Some(format!(
                "{} {}",
                record.metadata.category, record.metadata.source_id
            ))
        })
        .ok_or(SearchMappingError::MissingRequiredField { field: "title" })
}

fn person_display_name(fields: &Map<String, Value>) -> Option<String> {
    let initials = optional_string(fields, "initialen");
    let call_sign = optional_string(fields, "roepnaam");
    let prefix = optional_string(fields, "tussenvoegsel");
    let surname = optional_string(fields, "achternaam");

    let first = call_sign.or(initials);
    let display = [first, prefix, surname]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if display.is_empty() {
        None
    } else {
        Some(display)
    }
}

fn source_url(
    record: &SearchSourceRecord,
    document_content: Option<&SearchDocumentContent>,
) -> Option<String> {
    optional_string(&record.fields, "enclosure_url")
        .or_else(|| document_content.map(|content| content.selected_source_url.clone()))
        .and_then(|value| present_string(Some(value.as_str())))
}

fn optional_string(fields: &Map<String, Value>, field: &'static str) -> Option<String> {
    fields
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| present_string(Some(value)))
}

fn present_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn require_option<T>(value: Option<&T>, field: &'static str) -> Result<(), SearchMappingError> {
    if value.is_some() {
        Ok(())
    } else {
        Err(SearchMappingError::MissingRequiredField { field })
    }
}

fn require_field(
    fields: &Map<String, Value>,
    field: &'static str,
) -> Result<(), SearchMappingError> {
    match fields.get(field) {
        Some(Value::Null) | None => Err(SearchMappingError::MissingRequiredField { field }),
        Some(Value::String(value)) if value.trim().is_empty() => {
            Err(SearchMappingError::MissingRequiredField { field })
        }
        Some(_) => Ok(()),
    }
}

fn metadata_text(
    fields: &Map<String, Value>,
    document_content: Option<&SearchDocumentContent>,
) -> Result<Vec<String>, SearchMappingError> {
    let mut values = Vec::new();
    for (field, value) in fields {
        if matches!(
            field.as_str(),
            "titel" | "onderwerp" | "samenvatting" | "datum" | "document_nummer" | "enclosure_url"
        ) {
            continue;
        }
        append_metadata_value(field, value, &mut values)?;
    }

    if let Some(content) = document_content {
        values.push(format!("official_source: {}", content.official_source));
        values.push(format!("extraction_status: {}", content.extraction_status));
        values.push(format!("validation_status: {}", content.validation_status));
        if let Some(content_type) = present_string(content.selected_source_content_type.as_deref())
        {
            values.push(format!("selected_source_content_type: {content_type}"));
        }
        if let Some(output_hash) = present_string(content.output_hash.as_deref()) {
            values.push(format!("output_hash: {output_hash}"));
        }
    }

    Ok(values)
}

fn append_metadata_value(
    field: &str,
    value: &Value,
    values: &mut Vec<String>,
) -> Result<(), SearchMappingError> {
    match value {
        Value::Null => Ok(()),
        Value::Bool(value) => {
            values.push(format!("{field}: {value}"));
            Ok(())
        }
        Value::Number(value) => {
            values.push(format!("{field}: {value}"));
            Ok(())
        }
        Value::String(value) => {
            if !value.trim().is_empty() {
                values.push(format!("{field}: {}", value.trim()));
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                append_metadata_value(&format!("{field}[{index}]"), item, values)?;
            }
            Ok(())
        }
        Value::Object(object) => {
            for (name, item) in object {
                append_metadata_value(&format!("{field}.{name}"), item, values)?;
            }
            Ok(())
        }
    }
}
