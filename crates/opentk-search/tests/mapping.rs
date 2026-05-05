use chrono::{TimeZone, Utc};
use opentk_search::{
    map_record_to_operation, SearchDocumentContent, SearchEntityKind, SearchEntityMetadata,
    SearchIndexOperation, SearchMappingError, SearchRelationLabel, SearchSourceRecord,
};
use serde_json::{Map, Value};
use uuid::Uuid;

#[test]
fn complete_document_record_maps_to_search_upsert() {
    let source_id = document_source_id();
    let record = complete_document_record();

    let SearchIndexOperation::Upsert(document) = map_record_to_operation(&record).unwrap() else {
        panic!("document record must upsert");
    };

    assert_eq!(
        document.id.as_str(),
        "Document_00000000-0000-0000-0000-000000000123"
    );
    assert_eq!(
        document.key.as_str(),
        "Document:00000000-0000-0000-0000-000000000123"
    );
    assert_eq!(document.source_category, "Document");
    assert_eq!(document.source_id, source_id);
    assert_eq!(document.title, "Vragenuur digitalisering");
    assert_eq!(document.document_number.as_deref(), Some("2026D01234"));
    assert_eq!(
        document.source_url.as_deref(),
        Some("https://example.invalid/document.pdf")
    );
    assert_eq!(
        document.extracted_text.as_deref(),
        Some("Plain text about digital public services.")
    );
    assert_eq!(
        document.extracted_html.as_deref(),
        Some("<main>HTML body about digital public services.</main>")
    );
    assert!(document
        .metadata_text
        .contains(&"content_type: application/pdf".to_string()));
    assert!(document
        .metadata_text
        .contains(&"content_length: 12345".to_string()));
    assert_eq!(
        document.relation_labels,
        vec!["kamerstukdossier Kamerstukdossier Digitalisering overheid"]
    );
}

#[test]
fn document_mapping_reports_missing_expected_indexed_fields() {
    let mut record = complete_document_record();
    record.fields.remove("content_length");

    let error = map_record_to_operation(&record).unwrap_err();

    assert_eq!(
        error,
        SearchMappingError::MissingRequiredField {
            field: "content_length"
        }
    );
}

#[test]
fn person_record_maps_name_metadata_and_relations_without_document_fields() {
    let source_id = Uuid::parse_str("00000000-0000-0000-0000-000000000789").unwrap();
    let record = SearchSourceRecord {
        metadata: SearchEntityMetadata {
            category: "Persoon".to_string(),
            source_id,
            latest_skiptoken: 77,
            deleted: false,
            source_updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 9, 0, 0).unwrap(),
            atom_updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 9, 5, 0).unwrap(),
        },
        fields: Map::from_iter([
            ("initialen".to_string(), Value::String("F.".to_string())),
            ("roepnaam".to_string(), Value::String("Fatima".to_string())),
            ("tussenvoegsel".to_string(), Value::String(String::new())),
            ("achternaam".to_string(), Value::String("Kaya".to_string())),
        ]),
        document_content: None,
        relations: vec![SearchRelationLabel {
            relation_name: "fractie".to_string(),
            target_category: "Fractie".to_string(),
            target_id: Uuid::parse_str("00000000-0000-0000-0000-000000000790").unwrap(),
            label: "D66".to_string(),
        }],
    };

    let SearchIndexOperation::Upsert(document) = map_record_to_operation(&record).unwrap() else {
        panic!("person record must upsert");
    };

    assert_eq!(document.entity_kind, SearchEntityKind::Person);
    assert_eq!(document.title, "Fatima Kaya");
    assert_eq!(document.document_number, None);
    assert_eq!(document.extracted_text, None);
    assert!(document
        .metadata_text
        .contains(&"initialen: F.".to_string()));
    assert!(document
        .metadata_text
        .contains(&"roepnaam: Fatima".to_string()));
    assert!(document
        .metadata_text
        .contains(&"achternaam: Kaya".to_string()));
    assert_eq!(document.relation_labels, vec!["fractie Fractie D66"]);
    assert_eq!(document.filter_categories, vec!["Persoon", "Fractie"]);
}

#[test]
fn delete_and_update_records_map_to_index_operations() {
    let mut deleted = complete_document_record();
    deleted.metadata.deleted = true;
    deleted.fields.clear();
    deleted.document_content = None;

    let delete_operation = map_record_to_operation(&deleted).unwrap();

    assert_eq!(
        delete_operation,
        SearchIndexOperation::Delete("Document_00000000-0000-0000-0000-000000000123".to_string())
    );

    let mut updated = complete_document_record();
    updated.metadata.latest_skiptoken = 99;
    updated.fields.insert(
        "titel".to_string(),
        Value::String("Gewijzigd vragenuur digitalisering".to_string()),
    );
    updated.document_content.as_mut().unwrap().extracted_text =
        Some("Updated extracted text for incremental indexing.".to_string());

    let SearchIndexOperation::Upsert(document) = map_record_to_operation(&updated).unwrap() else {
        panic!("updated record must upsert");
    };

    assert_eq!(document.latest_skiptoken, 99);
    assert_eq!(document.title, "Gewijzigd vragenuur digitalisering");
    assert_eq!(
        document.extracted_text.as_deref(),
        Some("Updated extracted text for incremental indexing.")
    );
}

fn complete_document_record() -> SearchSourceRecord {
    let source_id = Uuid::parse_str("00000000-0000-0000-0000-000000000123").unwrap();
    SearchSourceRecord {
        metadata: SearchEntityMetadata {
            category: "Document".to_string(),
            source_id,
            latest_skiptoken: 42,
            deleted: false,
            source_updated_at: Utc.with_ymd_and_hms(2026, 4, 20, 12, 0, 0).unwrap(),
            atom_updated_at: Utc.with_ymd_and_hms(2026, 4, 20, 12, 5, 0).unwrap(),
        },
        fields: Map::from_iter([
            (
                "titel".to_string(),
                Value::String("Vragenuur digitalisering".to_string()),
            ),
            (
                "document_nummer".to_string(),
                Value::String("2026D01234".to_string()),
            ),
            ("datum".to_string(), Value::String("2026-04-20".to_string())),
            (
                "content_type".to_string(),
                Value::String("application/pdf".to_string()),
            ),
            ("content_length".to_string(), Value::Number(12_345.into())),
            (
                "enclosure_url".to_string(),
                Value::String("https://example.invalid/document.pdf".to_string()),
            ),
        ]),
        document_content: Some(SearchDocumentContent {
            selected_source_url: "https://example.invalid/document.pdf".to_string(),
            selected_source_content_type: Some("application/pdf".to_string()),
            official_source: true,
            extraction_status: "success".to_string(),
            validation_status: "valid".to_string(),
            output_hash: Some("sha256:abc".to_string()),
            extracted_text: Some("Plain text about digital public services.".to_string()),
            extracted_html: Some(
                "<main>HTML body about digital public services.</main>".to_string(),
            ),
        }),
        relations: vec![SearchRelationLabel {
            relation_name: "kamerstukdossier".to_string(),
            target_category: "Kamerstukdossier".to_string(),
            target_id: Uuid::parse_str("00000000-0000-0000-0000-000000000456").unwrap(),
            label: "Digitalisering overheid".to_string(),
        }],
    }
}

fn document_source_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000123").unwrap()
}
