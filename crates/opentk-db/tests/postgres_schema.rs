use std::collections::{HashMap, HashSet};

use opentk_core::official_schema::{self, FieldKind, Occurs};
use opentk_db::postgres_schema::{self, ColumnSpec, IndexPurpose, SqlType, TableKind, TableSpec};

#[test]
fn document_schema_exposes_metadata_scalar_and_relation_tables() {
    let schema = postgres_schema::schema();
    assert_columns(
        table(&schema.tables, "sync_category"),
        &[
            ("source_category", SqlType::Text, false),
            ("latest_skiptoken", SqlType::BigInteger, false),
            ("last_synced_at", SqlType::TimestampTz, true),
            ("next_url", SqlType::Text, true),
            ("resume_url", SqlType::Text, true),
            ("state", SqlType::Text, false),
            ("last_fetch_at", SqlType::TimestampTz, true),
            ("caught_up_at", SqlType::TimestampTz, true),
        ],
    );
    assert_columns(
        table(&schema.tables, "ingest_error"),
        &[
            ("id", SqlType::BigIdentity, false),
            ("phase", SqlType::Text, false),
            ("source_category", SqlType::Text, false),
            ("source_id", SqlType::Uuid, true),
            ("latest_skiptoken", SqlType::BigInteger, true),
            ("message", SqlType::Text, false),
            ("payload", SqlType::Jsonb, true),
            ("created_at", SqlType::TimestampTz, false),
        ],
    );

    let document = table(&schema.tables, "document");
    assert_eq!(
        document.kind,
        TableKind::Entity {
            category: "Document"
        }
    );
    assert_columns(
        document,
        &[
            ("source_category", SqlType::Text, false),
            ("source_id", SqlType::Uuid, false),
            ("latest_skiptoken", SqlType::BigInteger, false),
            ("deleted", SqlType::Boolean, false),
            ("source_updated_at", SqlType::TimestampTz, false),
            ("atom_updated_at", SqlType::TimestampTz, false),
            ("document_nummer", SqlType::Text, true),
            ("enclosure_url", SqlType::Text, true),
        ],
    );

    let relation = table(&schema.tables, "document__kamerstukdossier");
    assert_eq!(
        relation.kind,
        TableKind::Relation {
            source_category: "Document",
            relation_name: "kamerstukdossier"
        }
    );
    assert_columns(
        relation,
        &[
            ("source_category", SqlType::Text, false),
            ("source_id", SqlType::Uuid, false),
            ("relation_name", SqlType::Text, false),
            ("target_category", SqlType::Text, false),
            ("target_id", SqlType::Uuid, false),
            ("ordinal", SqlType::Integer, false),
            ("source_updated_at", SqlType::TimestampTz, false),
        ],
    );
}

#[test]
fn every_official_entity_has_one_entity_table() {
    let schema = postgres_schema::schema();
    for entity in official_schema::entity_types() {
        let tables: Vec<_> = schema
            .tables
            .iter()
            .filter(|table| {
                table.kind
                    == TableKind::Entity {
                        category: entity.category,
                    }
            })
            .collect();
        assert_eq!(
            tables.len(),
            1,
            "{} must have exactly one generated entity table",
            entity.category
        );
    }
}

#[test]
fn every_official_scalar_field_has_a_column_or_repeated_scalar_table() {
    let schema = postgres_schema::schema();
    let tables_by_name: HashMap<_, _> = schema
        .tables
        .iter()
        .map(|table| (table.name.as_str(), table))
        .collect();

    for entity in official_schema::entity_types() {
        let entity_table = table(&schema.tables, &postgres_schema::sql_name(entity.category));

        for base_attribute in entity.base_attributes {
            if ["id", "verwijderd", "bijgewerkt"].contains(&base_attribute.name) {
                continue;
            }
            assert!(
                entity_table.has_column(&postgres_schema::sql_name(base_attribute.name)),
                "{}.{} base attribute must be queryable as an entity-table column",
                entity.category,
                base_attribute.name
            );
        }

        for field in entity
            .fields
            .iter()
            .filter(|field| field.kind == FieldKind::Attribute)
        {
            let field_name = postgres_schema::sql_name(field.name);
            match field.max_occurs {
                Occurs::Exactly(1) => assert!(
                    entity_table.has_column(&field_name),
                    "{}.{} must be an entity-table column",
                    entity.category,
                    field.name
                ),
                Occurs::Exactly(_) | Occurs::Unbounded => {
                    let side_table_name = format!(
                        "{}__{}",
                        postgres_schema::sql_name(entity.category),
                        field_name
                    );
                    let side_table =
                        tables_by_name
                            .get(side_table_name.as_str())
                            .unwrap_or_else(|| {
                                panic!(
                                    "{}.{} must have repeated scalar table {}",
                                    entity.category, field.name, side_table_name
                                )
                            });
                    assert!(
                        side_table.has_column("ordinal") && side_table.has_column("value"),
                        "{side_table_name} must include ordinal and value columns"
                    );
                }
            }
        }
    }
}

#[test]
fn every_official_relation_has_queryable_table_constraints_and_indexes() {
    let schema = postgres_schema::schema();
    let index_purposes: HashSet<_> = schema
        .indexes
        .iter()
        .map(|index| (&index.table_name, index.purpose))
        .collect();

    for entity in official_schema::entity_types() {
        for field in entity
            .fields
            .iter()
            .filter(|field| field.kind == FieldKind::Relation)
        {
            let table_name = format!(
                "{}__{}",
                postgres_schema::sql_name(entity.category),
                postgres_schema::sql_name(field.name)
            );
            let relation = table(&schema.tables, &table_name);
            assert_columns(
                relation,
                &[
                    ("source_category", SqlType::Text, false),
                    ("source_id", SqlType::Uuid, false),
                    ("relation_name", SqlType::Text, false),
                    ("target_category", SqlType::Text, false),
                    ("target_id", SqlType::Uuid, false),
                    ("ordinal", SqlType::Integer, false),
                    ("source_updated_at", SqlType::TimestampTz, false),
                ],
            );
            assert!(
                !relation.primary_key.is_empty(),
                "{table_name} must have a primary key"
            );
            assert!(
                relation
                    .foreign_keys
                    .iter()
                    .any(|fk| fk.referenced_table == postgres_schema::sql_name(entity.category)),
                "{table_name} must foreign-key to its source entity table"
            );
            assert!(
                relation
                    .foreign_keys
                    .iter()
                    .any(|fk| fk.referenced_table == "sync_entity"),
                "{table_name} must have a target registry foreign key"
            );
            if field.max_occurs == Occurs::Exactly(1) {
                assert!(
                    !relation.unique_constraints.is_empty(),
                    "{table_name} must enforce single relation multiplicity"
                );
            }
            assert!(
                index_purposes.contains(&(&table_name, IndexPurpose::RelationSource))
                    && index_purposes.contains(&(&table_name, IndexPurpose::RelationTarget)),
                "{table_name} must have source and target endpoint indexes"
            );
        }
    }
}

#[test]
fn required_direct_query_paths_are_indexed() {
    let schema = postgres_schema::schema();
    let purposes: HashSet<_> = schema
        .indexes
        .iter()
        .map(|index| (index.table_name.as_str(), index.purpose))
        .collect();

    assert!(purposes.contains(&("sync_category", IndexPurpose::CategoryCursor)));
    assert!(purposes.contains(&("sync_entity", IndexPurpose::PrimaryUuidLookup)));
    assert!(purposes.contains(&("sync_entity", IndexPurpose::EntityUpdatedAt)));
    assert!(purposes.contains(&("document", IndexPurpose::DocumentNumber)));
    assert!(purposes.contains(&("document", IndexPurpose::AssetOwner)));

    assert!(
        purposes
            .iter()
            .any(|(_, purpose)| *purpose == IndexPurpose::DateScan),
        "at least one official date/timestamp field must have a date-scan index"
    );
}

#[test]
fn document_asset_schema_preserves_upstream_link_metadata_and_retrieval_state() {
    let schema = postgres_schema::schema();
    let asset = table(&schema.tables, "document_asset");

    assert_eq!(asset.kind, TableKind::DocumentAsset);
    assert_columns(
        asset,
        &[
            ("id", SqlType::BigIdentity, false),
            ("document_source_category", SqlType::Text, false),
            ("document_source_id", SqlType::Uuid, false),
            ("asset_url", SqlType::Text, false),
            ("upstream_url", SqlType::Text, false),
            ("upstream_content_type", SqlType::Text, true),
            ("upstream_content_length", SqlType::BigInteger, true),
            ("upstream_last_modified_at", SqlType::TimestampTz, true),
            ("retrieval_status", SqlType::Text, false),
            ("retrieval_error", SqlType::Text, true),
            ("retrieved_at", SqlType::TimestampTz, true),
            ("created_at", SqlType::TimestampTz, false),
            ("updated_at", SqlType::TimestampTz, false),
        ],
    );
    assert_eq!(asset.primary_key, ["id"]);
    assert!(
        asset.foreign_keys.iter().any(|fk| {
            fk.referenced_table == "document"
                && fk.columns == ["document_source_category", "document_source_id"]
                && fk.referenced_columns == ["source_category", "source_id"]
        }),
        "document_asset must cascade from its owning document"
    );
    assert!(
        asset.unique_constraints.iter().any(|unique| unique.columns
            == [
                "document_source_category",
                "document_source_id",
                "asset_url"
            ]),
        "document_asset must deduplicate asset URLs per document"
    );

    let purposes: HashSet<_> = schema
        .indexes
        .iter()
        .map(|index| {
            (
                index.table_name.as_str(),
                index.purpose,
                index.columns.as_slice(),
            )
        })
        .collect();

    assert!(purposes.contains(&(
        "document_asset",
        IndexPurpose::AssetOwner,
        [
            "document_source_category".to_owned(),
            "document_source_id".to_owned()
        ]
        .as_slice(),
    )));
    assert!(purposes.contains(&(
        "document_asset",
        IndexPurpose::AssetUrl,
        ["asset_url".to_owned()].as_slice(),
    )));
    assert!(purposes.contains(&(
        "document_asset",
        IndexPurpose::AssetUrl,
        ["upstream_url".to_owned()].as_slice(),
    )));
}

#[test]
fn document_content_schema_stores_official_source_selection_and_extraction_provenance() {
    let schema = postgres_schema::schema();
    let content = table(&schema.tables, "document_content");

    assert_eq!(content.kind, TableKind::DocumentContent);
    assert_columns(
        content,
        &[
            ("id", SqlType::BigIdentity, false),
            ("document_asset_id", SqlType::BigInteger, false),
            ("document_source_category", SqlType::Text, false),
            ("document_source_id", SqlType::Uuid, false),
            ("selected_source_url", SqlType::Text, false),
            ("selected_source_content_type", SqlType::Text, true),
            ("selected_source_content_length", SqlType::BigInteger, true),
            ("official_source", SqlType::Boolean, false),
            ("source_rank", SqlType::Integer, false),
            ("extraction_status", SqlType::Text, false),
            ("validation_status", SqlType::Text, false),
            ("extraction_tool", SqlType::Text, false),
            ("extraction_tool_version", SqlType::Text, false),
            ("source_hash", SqlType::Text, false),
            ("output_hash", SqlType::Text, true),
            ("extraction_error", SqlType::Text, true),
            ("extracted_text", SqlType::Text, true),
            ("extracted_html", SqlType::Text, true),
            ("extracted_at", SqlType::TimestampTz, false),
            ("created_at", SqlType::TimestampTz, false),
            ("updated_at", SqlType::TimestampTz, false),
        ],
    );
    assert_eq!(content.primary_key, ["id"]);
    assert!(
        content.foreign_keys.iter().any(|fk| {
            fk.referenced_table == "document_asset"
                && fk.columns == ["document_asset_id"]
                && fk.referenced_columns == ["id"]
        }),
        "document_content must cascade from the asset extraction source"
    );
    assert!(
        content.foreign_keys.iter().any(|fk| {
            fk.referenced_table == "document"
                && fk.columns == ["document_source_category", "document_source_id"]
                && fk.referenced_columns == ["source_category", "source_id"]
        }),
        "document_content must keep a direct document owner path"
    );
    assert!(
        content
            .unique_constraints
            .iter()
            .any(|unique| unique.columns == ["document_asset_id", "source_hash", "output_hash"]),
        "document_content must deduplicate extracted bodies per source/output hash"
    );

    let purposes: HashSet<_> = schema
        .indexes
        .iter()
        .map(|index| {
            (
                index.table_name.as_str(),
                index.purpose,
                index.columns.as_slice(),
            )
        })
        .collect();

    assert!(purposes.contains(&(
        "document_content",
        IndexPurpose::DocumentContentOwner,
        [
            "document_source_category".to_owned(),
            "document_source_id".to_owned()
        ]
        .as_slice(),
    )));
    assert!(purposes.contains(&(
        "document_content",
        IndexPurpose::ForeignKeyPath,
        ["document_asset_id".to_owned()].as_slice(),
    )));
    assert!(purposes.contains(&(
        "document_content",
        IndexPurpose::OfficialContentSource,
        ["official_source".to_owned(), "source_rank".to_owned()].as_slice(),
    )));
}

#[test]
fn document_content_schema_constrains_status_values_and_body_consistency() {
    let schema = postgres_schema::schema();
    let asset = table(&schema.tables, "document_asset");
    let content = table(&schema.tables, "document_content");

    assert!(
        asset.check_constraints.iter().any(|constraint| {
            constraint.name == "document_asset_retrieval_status_check"
                && constraint.expression
                    == "retrieval_status IN ('pending', 'fetched', 'not_found', 'unsupported_content_type', 'failed')"
        }),
        "document_asset must constrain retrieval_status to known states"
    );
    assert!(
        content.check_constraints.iter().any(|constraint| {
            constraint.name == "document_content_extraction_status_check"
                && constraint.expression
                    == "extraction_status IN ('pending', 'extracted', 'empty', 'failed')"
        }),
        "document_content must constrain extraction_status to known states"
    );
    assert!(
        content.check_constraints.iter().any(|constraint| {
            constraint.name == "document_content_validation_status_check"
                && constraint.expression
                    == "validation_status IN ('unverified', 'valid', 'invalid')"
        }),
        "document_content must constrain validation_status to known states"
    );
    assert!(
        content.check_constraints.iter().any(|constraint| {
            constraint.name == "document_content_extracted_body_check"
                && constraint.expression
                    == "(extraction_status = 'failed' AND extraction_error IS NOT NULL) OR (extraction_status <> 'failed' AND (extracted_text IS NOT NULL OR extracted_html IS NOT NULL))"
        }),
        "document_content must reject bodyless rows unless they are explicit failures"
    );
}

#[test]
fn checked_in_migrations_match_schema_spec() {
    let schema = postgres_schema::schema();
    let up_sql = include_str!("../../../migrations/20260426000000_sync_schema.up.sql");
    let down_sql = include_str!("../../../migrations/20260426000000_sync_schema.down.sql");

    assert_eq!(up_sql, postgres_schema::render_up_migration(&schema));
    assert_eq!(down_sql, postgres_schema::render_down_migration(&schema));
}

fn table<'a>(tables: &'a [TableSpec], name: &str) -> &'a TableSpec {
    tables
        .iter()
        .find(|table| table.name == name)
        .unwrap_or_else(|| panic!("table {name} is present"))
}

fn assert_columns(table: &TableSpec, expected: &[(&str, SqlType, bool)]) {
    for (name, sql_type, nullable) in expected {
        let column = column(table, name);
        assert_eq!(
            (&column.sql_type, column.nullable),
            (sql_type, *nullable),
            "{}.{} has expected type/nullability",
            table.name,
            name
        );
    }
}

fn column<'a>(table: &'a TableSpec, name: &str) -> &'a ColumnSpec {
    table
        .columns
        .iter()
        .find(|column| column.name == name)
        .unwrap_or_else(|| panic!("{}.{} is present", table.name, name))
}
