use opentk_search::meilisearch_schema;

#[test]
fn meilisearch_schema_declares_search_result_and_filter_contract() {
    let schema = meilisearch_schema();

    assert_eq!(schema.index_name, "opentk_entities");
    assert_eq!(schema.primary_key, "id");
    assert_eq!(
        schema.searchable_attributes,
        vec![
            "title",
            "summary",
            "document_number",
            "metadata_text",
            "relation_labels",
            "extracted_text",
            "extracted_html",
        ]
    );
    assert!(schema.displayed_attributes.contains(&"source_id"));
    assert!(schema.displayed_attributes.contains(&"entity_kind"));
    assert!(schema.displayed_attributes.contains(&"_formatted"));
    assert_eq!(
        schema.filterable_attributes,
        vec![
            "source_category",
            "entity_kind",
            "date",
            "filter_categories",
            "latest_skiptoken",
        ]
    );
    assert_eq!(
        schema.sortable_attributes,
        vec!["date", "source_updated_at", "latest_skiptoken"]
    );
    assert_eq!(
        schema.ranking_rules,
        vec![
            "words",
            "typo",
            "proximity",
            "attribute",
            "sort",
            "exactness"
        ]
    );
}
