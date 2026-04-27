use opentk_core::official_schema;
use opentk_sync::{
    payload::{parse_entity_xml, parse_entry_payload, ParsedValue},
    syncfeed::{SyncFeedContentMode, SyncFeedCursor, SyncFeedEntry},
};
use reqwest::Url;
use uuid::Uuid;

#[test]
fn document_payload_parses_scalars_relation_and_entry_enclosure() {
    let entry = SyncFeedEntry {
        id: "entry-1".to_owned(),
        category: "Document".to_owned(),
        updated: "2026-04-26T00:00:00Z".to_owned(),
        next: SyncFeedCursor::first_page(
            &Url::parse("https://example.test").expect("valid base URL"),
            "Document",
            SyncFeedContentMode::Internal,
        ),
        content_xml: Some(include_str!("fixtures/syncfeed_payloads/document.xml").to_owned()),
        enclosure_url: Some(
            Url::parse("https://example.test/document.bin").expect("valid enclosure URL"),
        ),
    };

    let parsed = parse_entry_payload(&entry).expect("document payload parses");

    assert_eq!(parsed.category, "Document");
    assert_eq!(parsed.xml_element, "document");
    assert_eq!(
        parsed.source_id,
        Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid")
    );
    assert!(!parsed.deleted);
    assert_eq!(parsed.content_type.as_deref(), Some("application/pdf"));
    assert_eq!(parsed.content_length, Some(12345));
    assert_eq!(
        parsed.enclosure_url.as_ref().map(Url::as_str),
        Some("https://example.test/document.bin")
    );
    assert!(parsed.scalars.iter().any(|scalar| {
        scalar.name == "documentNummer"
            && scalar.value == ParsedValue::Text("2026D00001".to_owned())
    }));
    assert!(parsed.relations.iter().any(|relation| {
        relation.name == "kamerstukdossier"
            && relation.target_category == "Kamerstukdossier"
            && relation.target_id
                == Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid")
    }));
}

#[test]
fn repeated_relations_keep_document_order_ordinals() {
    let parsed = parse_entity_xml(
        "Document",
        include_str!("fixtures/syncfeed_payloads/document_repeated_relations.xml"),
    )
    .expect("document payload parses");

    let activiteit: Vec<_> = parsed
        .relations
        .iter()
        .filter(|relation| relation.name == "activiteit")
        .collect();
    assert_eq!(activiteit.len(), 2);
    assert_eq!(activiteit[0].ordinal, 1);
    assert_eq!(activiteit[1].ordinal, 2);
    assert_eq!(
        activiteit[0].target_id,
        Uuid::parse_str("33333333-3333-4333-8333-333333333331").expect("valid uuid")
    );
    assert_eq!(
        activiteit[1].target_id,
        Uuid::parse_str("33333333-3333-4333-8333-333333333332").expect("valid uuid")
    );
}

#[test]
fn delete_marker_carries_metadata_and_discards_current_body_content() {
    let deleted = parse_entity_xml(
        "Document",
        r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="11111111-1111-4111-8111-111111111111"
            verwijderd="true"
            bijgewerkt="2026-04-26T00:00:00Z"/>"#,
    )
    .expect("delete marker parses");

    assert!(deleted.deleted);
    assert!(deleted.scalars.is_empty());
    assert!(deleted.relations.is_empty());

    let deleted_with_body = parse_entity_xml(
        "Document",
        r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="11111111-1111-4111-8111-111111111111"
            verwijderd="true"
            bijgewerkt="2026-04-26T00:00:00Z">
            <documentNummer>2026D00001</documentNummer>
        </document>"#,
    )
    .expect("deleted entity body parses as tombstone");
    assert!(deleted_with_body.deleted);
    assert_eq!(deleted_with_body.source_id, deleted.source_id);
    assert!(deleted_with_body.scalars.is_empty());
    assert!(deleted_with_body.relations.is_empty());
}

#[test]
fn live_persoon_capitalized_deleted_boolean_parses() {
    let deleted = parse_entity_xml(
        "Persoon",
        r#"<persoon xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="11111111-1111-4111-8111-111111111111"
            verwijderd="True"
            bijgewerkt="2026-04-26T00:00:00Z"/>"#,
    )
    .expect("live Persoon delete marker parses");

    assert_eq!(deleted.category, "Persoon");
    assert!(deleted.deleted);
}

#[test]
fn live_syncfeed_naive_fractional_datetimes_parse_as_utc() {
    let deleted = parse_entity_xml(
        "Document",
        r#"<document xmlns:tk="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="11111111-1111-4111-8111-111111111111"
            tk:verwijderd="true"
            tk:bijgewerkt="2019-06-28T23:59:41.5570000"/>"#,
    )
    .expect("live delete marker parses");

    assert_eq!(
        deleted.source_updated_at.to_rfc3339(),
        "2019-06-28T23:59:41.557+00:00"
    );
}

#[test]
fn toezegging_live_toegezegd_aan_relation_parses() {
    let parsed = parse_entity_xml(
        "Toezegging",
        r#"<toezegging xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="66666666-6666-4666-8666-666666666666"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <toegezegdAan ref="77777777-7777-4777-8777-777777777777"/>
            <tekst>De toezeggingstekst blijft als scalar beschikbaar.</tekst>
        </toezegging>"#,
    )
    .expect("live Toezegging payload parses");

    assert_eq!(parsed.category, "Toezegging");
    assert_eq!(parsed.xml_element, "toezegging");
    assert!(parsed.relations.iter().any(|relation| {
        relation.name == "toegezegdAan"
            && relation.target_id
                == Uuid::parse_str("77777777-7777-4777-8777-777777777777").expect("valid uuid")
    }));
    assert!(parsed.scalars.iter().any(|scalar| {
        scalar.name == "tekst"
            && scalar.value
                == ParsedValue::Text(
                    "De toezeggingstekst blijft als scalar beschikbaar.".to_owned(),
                )
    }));
}

#[test]
fn toezegging_live_repeated_toegezegd_aan_relations_keep_ordinals() {
    let parsed = parse_entity_xml(
        "Toezegging",
        r#"<toezegging xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="77777777-7777-4777-8777-777777777777"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <toegezegdAan ref="88888888-8888-4888-8888-888888888881"/>
            <toegezegdAan ref="88888888-8888-4888-8888-888888888882"/>
            <tekst>De live feed kan meerdere ontvangers voor dezelfde toezegging leveren.</tekst>
        </toezegging>"#,
    )
    .expect("live Toezegging payload with repeated toegezegdAan parses");

    let toegezegd_aan = parsed
        .relations
        .iter()
        .filter(|relation| relation.name == "toegezegdAan")
        .collect::<Vec<_>>();
    assert_eq!(toegezegd_aan.len(), 2);
    assert_eq!(toegezegd_aan[0].ordinal, 1);
    assert_eq!(
        toegezegd_aan[0].target_id,
        Uuid::parse_str("88888888-8888-4888-8888-888888888881").expect("valid uuid")
    );
    assert_eq!(toegezegd_aan[1].ordinal, 2);
    assert_eq!(
        toegezegd_aan[1].target_id,
        Uuid::parse_str("88888888-8888-4888-8888-888888888882").expect("valid uuid")
    );
}

#[test]
fn toezegging_live_empty_toegezegd_aan_relation_is_absent() {
    let parsed = parse_entity_xml(
        "Toezegging",
        r#"<toezegging xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="88888888-8888-4888-8888-888888888888"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <toegezegdAan/>
            <tekst>Een lege toegezegdAan uit de live feed blokkeert parsing niet.</tekst>
        </toezegging>"#,
    )
    .expect("live Toezegging payload with empty toegezegdAan parses");

    assert_eq!(parsed.category, "Toezegging");
    assert!(!parsed
        .relations
        .iter()
        .any(|relation| relation.name == "toegezegdAan"));
    assert!(parsed.scalars.iter().any(|scalar| {
        scalar.name == "tekst"
            && scalar.value
                == ParsedValue::Text(
                    "Een lege toegezegdAan uit de live feed blokkeert parsing niet.".to_owned(),
                )
    }));
}

#[test]
fn toezegging_live_expanded_empty_toegezegd_aan_relation_is_absent() {
    let parsed = parse_entity_xml(
        "Toezegging",
        r#"<toezegging xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="99999999-9999-4999-8999-999999999999"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <toegezegdAan></toegezegdAan>
            <tekst>Een uitgebreid lege toegezegdAan blokkeert parsing niet.</tekst>
        </toezegging>"#,
    )
    .expect("live Toezegging payload with expanded empty toegezegdAan parses");

    assert_eq!(parsed.category, "Toezegging");
    assert!(!parsed
        .relations
        .iter()
        .any(|relation| relation.name == "toegezegdAan"));
    assert!(parsed.scalars.iter().any(|scalar| {
        scalar.name == "tekst"
            && scalar.value
                == ParsedValue::Text(
                    "Een uitgebreid lege toegezegdAan blokkeert parsing niet.".to_owned(),
                )
    }));
}

#[test]
fn toezegging_live_duplicate_kamerbrief_nakoming_scalars_keep_ordinals() {
    let parsed = parse_entity_xml(
        "Toezegging",
        r#"<toezegging xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <kamerbriefNakoming>2026Z00001</kamerbriefNakoming>
            <kamerbriefNakoming>2026Z00002</kamerbriefNakoming>
            <tekst>De live feed kan meerdere Kamerbrieven voor dezelfde toezegging leveren.</tekst>
        </toezegging>"#,
    )
    .expect("live Toezegging payload with duplicate kamerbriefNakoming parses");

    let kamerbrieven = parsed
        .scalars
        .iter()
        .filter(|scalar| scalar.name == "kamerbriefNakoming")
        .collect::<Vec<_>>();
    assert_eq!(kamerbrieven.len(), 2);
    assert_eq!(
        kamerbrieven[0].value,
        ParsedValue::Text("2026Z00001".to_owned())
    );
    assert_eq!(kamerbrieven[0].ordinal, 1);
    assert_eq!(
        kamerbrieven[1].value,
        ParsedValue::Text("2026Z00002".to_owned())
    );
    assert_eq!(kamerbrieven[1].ordinal, 2);
}

#[test]
fn scalar_datatypes_are_validated_and_typed() {
    let activiteit = parse_entity_xml(
        "Activiteit",
        include_str!("fixtures/syncfeed_payloads/activiteit_scalars.xml"),
    )
    .expect("activiteit payload parses");
    assert!(activiteit
        .scalars
        .iter()
        .any(|scalar| { scalar.name == "besloten" && scalar.value == ParsedValue::Bool(true) }));
    assert!(activiteit.scalars.iter().any(|scalar| {
        matches!(scalar.value, ParsedValue::DateTime(_)) && scalar.name == "datum"
    }));

    let dossier = parse_entity_xml(
        "Kamerstukdossier",
        include_str!("fixtures/syncfeed_payloads/kamerstukdossier_scalars.xml"),
    )
    .expect("kamerstukdossier payload parses");
    assert!(dossier
        .scalars
        .iter()
        .any(|scalar| scalar.name == "nummer" && scalar.value == ParsedValue::I32(36600)));

    let invalid = parse_entity_xml(
        "Activiteit",
        r#"<activiteit xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="44444444-4444-4444-8444-444444444444"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <besloten>sometimes</besloten>
        </activiteit>"#,
    )
    .expect_err("invalid boolean is rejected");
    assert!(invalid.to_string().contains("invalid xs:boolean"));
}

#[test]
fn every_official_entity_accepts_minimal_payload() {
    for entity in official_schema::entity_types() {
        let xml = format!(
            r#"<{root} xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0" id="55555555-5555-4555-8555-555555555555" verwijderd="false" bijgewerkt="2026-04-26T00:00:00Z"/>"#,
            root = entity.xml_element
        );
        let parsed = parse_entity_xml(entity.category, &xml)
            .unwrap_or_else(|error| panic!("{} minimal payload parses: {error}", entity.category));
        assert_eq!(parsed.category, entity.category);
    }
}
