use opentk_core::official_schema::{self, FieldKind, Occurs};

#[test]
fn document_schema_exposes_official_field_and_relation_metadata() {
    let document = official_schema::entity_named("Document").expect("Document model is present");

    assert_eq!(document.category, "Document");
    assert_eq!(document.xml_element, "document");
    assert_eq!(document.base, "downloadEntiteitType");

    let kamerstukdossier = document
        .field_named("kamerstukdossier")
        .expect("Document.kamerstukdossier relation is present");
    assert_eq!(kamerstukdossier.order, 6);
    assert_eq!(kamerstukdossier.kind, FieldKind::Relation);
    assert_eq!(kamerstukdossier.xsd_type, "referentieLiteral");
    assert_eq!(kamerstukdossier.min_occurs, 0);
    assert_eq!(kamerstukdossier.max_occurs, Occurs::Exactly(1));
    assert!(!kamerstukdossier.nillable);

    let titel = document
        .field_named("titel")
        .expect("Document.titel scalar field is present");
    assert_eq!(titel.order, 9);
    assert_eq!(titel.kind, FieldKind::Attribute);
    assert_eq!(titel.xsd_type, "tokenType");
    assert_eq!(titel.min_occurs, 0);
    assert_eq!(titel.max_occurs, Occurs::Exactly(1));
    assert!(titel.nillable);
}
