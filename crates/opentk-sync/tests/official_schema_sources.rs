use opentk_core::official_schema::{self, FieldKind, Occurs};
use opentk_sync::official_schema::{
    extract_entities_from_dir, official_xsd_dir, source_gap_report, verify_checked_in_model,
    verify_model_against_dir, TASK_DOCUMENTED_SOURCE_GAPS,
};
use std::{env, fs};

#[test]
fn document_model_matches_vendored_official_xsd() {
    let sources = extract_entities_from_dir(official_xsd_dir()).expect("official XSDs parse");
    let document_source = sources
        .iter()
        .find(|entity| entity.category == "Document")
        .expect("Document source XSD exists");
    let document_model = official_schema::entity_named("Document").expect("Document model exists");

    assert_eq!(document_model.xml_element, document_source.xml_element);
    assert_eq!(document_model.base, document_source.base);

    let kamerstukdossier = document_source
        .fields
        .iter()
        .find(|field| field.name == "kamerstukdossier")
        .expect("Document.kamerstukdossier exists in XSD");
    assert_eq!(kamerstukdossier.order, 6);
    assert_eq!(kamerstukdossier.kind, FieldKind::Relation);
    assert_eq!(kamerstukdossier.xsd_type, "referentieLiteral");
    assert_eq!(kamerstukdossier.min_occurs, 0);
    assert_eq!(kamerstukdossier.max_occurs, Occurs::Exactly(1));
    assert!(!kamerstukdossier.nillable);

    let titel = document_source
        .fields
        .iter()
        .find(|field| field.name == "titel")
        .expect("Document.titel exists in XSD");
    assert_eq!(titel.order, 9);
    assert_eq!(titel.kind, FieldKind::Attribute);
    assert_eq!(titel.xsd_type, "tokenType");
    assert_eq!(titel.min_occurs, 0);
    assert_eq!(titel.max_occurs, Occurs::Exactly(1));
    assert!(titel.nillable);
}

#[test]
fn all_vendored_official_entities_have_matching_model_entries() {
    let mismatches = verify_checked_in_model().expect("official XSDs parse");

    assert_eq!(mismatches, Vec::new());
}

#[test]
fn every_planned_syncfeed_category_has_model_entry() {
    let sources = extract_entities_from_dir(official_xsd_dir()).expect("official XSDs parse");

    for source in sources {
        assert!(
            official_schema::entity_named(&source.category).is_some(),
            "{} should have a model entry",
            source.category
        );
    }
}

#[test]
fn task_documented_categories_missing_from_pinned_sources_are_reported() {
    let report = source_gap_report();

    for gap in TASK_DOCUMENTED_SOURCE_GAPS {
        assert!(report.contains(gap.category));
        assert!(report.contains(gap.reason));
        assert!(
            official_schema::entity_named(gap.category).is_none(),
            "{} should stay out of the model until an official XSD exists",
            gap.category
        );
    }
}

#[test]
fn schema_check_reports_a_clear_mismatch_when_a_source_file_is_missing() {
    let fixture_dir = env::temp_dir().join(format!(
        "opentk-official-schema-missing-document-{}",
        std::process::id()
    ));
    if fixture_dir.exists() {
        fs::remove_dir_all(&fixture_dir).expect("old fixture dir can be removed");
    }
    copy_dir(official_xsd_dir(), &fixture_dir);
    fs::remove_file(fixture_dir.join("tkData-v1-0-document.xsd"))
        .expect("fixture document XSD can be removed");

    let mismatches = verify_model_against_dir(&fixture_dir).expect("mutated official XSDs parse");

    assert!(
        mismatches.iter().any(|mismatch| {
            mismatch.category == "Document"
                && mismatch
                    .detail
                    .contains("model entity has no vendored official XSD")
        }),
        "mismatches should explain the missing source file: {mismatches:?}"
    );

    fs::remove_dir_all(&fixture_dir).expect("fixture dir can be removed");
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).expect("fixture dir can be created");
    for entry in fs::read_dir(from).expect("official XSD dir can be read") {
        let entry = entry.expect("official XSD dir entry can be read");
        let source = entry.path();
        let destination = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &destination);
        } else {
            fs::copy(&source, destination).expect("official XSD file can be copied");
        }
    }
}
