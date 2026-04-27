use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use opentk_core::official_schema::{self, EntityType, Field, FieldKind, Occurs};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaSpec {
    pub tables: Vec<TableSpec>,
    pub indexes: Vec<IndexSpec>,
}

impl SchemaSpec {
    #[must_use]
    pub fn table_named(&self, name: &str) -> Option<&TableSpec> {
        self.tables.iter().find(|table| table.name == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableSpec {
    pub name: String,
    pub kind: TableKind,
    pub columns: Vec<ColumnSpec>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKeySpec>,
    pub unique_constraints: Vec<UniqueConstraintSpec>,
    pub check_constraints: Vec<CheckConstraintSpec>,
}

impl TableSpec {
    #[must_use]
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|column| column.name == name)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TableKind {
    SyncMetadata,
    EntityRegistry,
    Entity {
        category: &'static str,
    },
    Relation {
        source_category: &'static str,
        relation_name: &'static str,
    },
    RepeatedScalar {
        category: &'static str,
        field_name: &'static str,
    },
    DocumentAsset,
    DocumentContent,
    SearchIndexCursor,
    SearchIndexFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColumnSpec {
    pub name: String,
    pub sql_type: SqlType,
    pub nullable: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SqlType {
    Uuid,
    Text,
    Boolean,
    Integer,
    BigInteger,
    BigIdentity,
    TimestampTz,
    Date,
    Jsonb,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignKeySpec {
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_delete: ForeignKeyAction,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ForeignKeyAction {
    Cascade,
    Restrict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UniqueConstraintSpec {
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckConstraintSpec {
    pub name: String,
    pub expression: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexSpec {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub purpose: IndexPurpose,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IndexPurpose {
    PrimaryUuidLookup,
    CategoryCursor,
    EntityUpdatedAt,
    DocumentNumber,
    DateScan,
    RelationSource,
    RelationTarget,
    AssetOwner,
    AssetUrl,
    DocumentContentOwner,
    OfficialContentSource,
    SearchIndexCursor,
    SearchIndexFailureRetry,
    SearchIndexFailureSource,
    ForeignKeyPath,
}

#[must_use]
pub fn schema() -> SchemaSpec {
    let mut tables = vec![
        sync_category_table(),
        ingest_error_table(),
        sync_entity_table(),
        search_index_cursor_table(),
        search_index_failure_table(),
    ];
    let mut indexes = vec![
        index(
            "sync_category",
            &["source_category", "latest_skiptoken"],
            false,
            IndexPurpose::CategoryCursor,
        ),
        index(
            "sync_entity",
            &["source_id"],
            false,
            IndexPurpose::PrimaryUuidLookup,
        ),
        index(
            "sync_entity",
            &["source_category", "source_updated_at"],
            false,
            IndexPurpose::EntityUpdatedAt,
        ),
        index(
            "sync_entity",
            &["source_category", "deleted", "latest_skiptoken"],
            false,
            IndexPurpose::CategoryCursor,
        ),
        index(
            "search_index_cursor",
            &["index_name", "source_category", "latest_skiptoken"],
            false,
            IndexPurpose::SearchIndexCursor,
        ),
        index(
            "search_index_failure",
            &["next_retry_at", "attempt_count"],
            false,
            IndexPurpose::SearchIndexFailureRetry,
        ),
        index(
            "search_index_failure",
            &[
                "index_name",
                "source_category",
                "source_id",
                "latest_skiptoken",
            ],
            false,
            IndexPurpose::SearchIndexFailureSource,
        ),
    ];

    for entity in official_schema::entity_types() {
        let entity_table = entity_table(entity);
        indexes.extend(entity_indexes(entity, &entity_table));
        tables.push(entity_table);

        for field in entity.fields {
            match field.kind {
                FieldKind::Attribute if is_repeated(field.max_occurs) => {
                    let table = repeated_scalar_table(entity, field);
                    indexes.push(index(
                        &table.name,
                        &["source_category", "source_id"],
                        false,
                        IndexPurpose::ForeignKeyPath,
                    ));
                    tables.push(table);
                }
                FieldKind::Relation => {
                    let table = relation_table(entity, field);
                    indexes.push(index(
                        &table.name,
                        &["source_category", "source_id", "relation_name"],
                        false,
                        IndexPurpose::RelationSource,
                    ));
                    indexes.push(index(
                        &table.name,
                        &["target_category", "target_id"],
                        false,
                        IndexPurpose::RelationTarget,
                    ));
                    indexes.push(index(
                        &table.name,
                        &["source_category", "source_id"],
                        false,
                        IndexPurpose::ForeignKeyPath,
                    ));
                    tables.push(table);
                }
                FieldKind::Attribute => {}
            }
        }
    }

    append_document_content_storage(&mut tables, &mut indexes);

    SchemaSpec { tables, indexes }
}

fn append_document_content_storage(tables: &mut Vec<TableSpec>, indexes: &mut Vec<IndexSpec>) {
    let document_asset = document_asset_table();
    indexes.push(index(
        &document_asset.name,
        &["document_source_category", "document_source_id"],
        false,
        IndexPurpose::AssetOwner,
    ));
    indexes.push(index(
        &document_asset.name,
        &["asset_url"],
        false,
        IndexPurpose::AssetUrl,
    ));
    indexes.push(index(
        &document_asset.name,
        &["upstream_url"],
        false,
        IndexPurpose::AssetUrl,
    ));
    tables.push(document_asset);

    let document_content = document_content_table();
    indexes.push(index(
        &document_content.name,
        &["document_source_category", "document_source_id"],
        false,
        IndexPurpose::DocumentContentOwner,
    ));
    indexes.push(index(
        &document_content.name,
        &["document_asset_id"],
        false,
        IndexPurpose::ForeignKeyPath,
    ));
    indexes.push(index(
        &document_content.name,
        &["official_source", "source_rank"],
        false,
        IndexPurpose::OfficialContentSource,
    ));
    tables.push(document_content);
}

#[must_use]
pub fn sql_name(official_name: &str) -> String {
    let mut output = String::new();
    let mut previous_was_lower_or_digit = false;

    for character in official_name.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase() && previous_was_lower_or_digit {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
            previous_was_lower_or_digit =
                character.is_ascii_lowercase() || character.is_ascii_digit();
        } else if !output.ends_with('_') {
            output.push('_');
            previous_was_lower_or_digit = false;
        }
    }

    output.trim_matches('_').to_owned()
}

#[must_use]
pub fn render_ensure_schema(schema: &SchemaSpec) -> String {
    let mut sql = String::from("-- Generated from opentk-db::postgres_schema.\n");
    sql.push_str("-- Idempotent schema bootstrap for binary-owned startup.\n\n");

    for table in &schema.tables {
        render_create_table_if_not_exists(&mut sql, table);
        sql.push('\n');
    }

    render_search_cdc_trigger_replacement(&mut sql);
    sql.push('\n');

    for index_spec in &schema.indexes {
        render_create_index_if_not_exists(&mut sql, index_spec);
    }

    sql
}

fn render_search_cdc_trigger_replacement(sql: &mut String) {
    sql.push_str(concat!(
        "CREATE OR REPLACE FUNCTION notify_sync_entity_change()\n",
        "RETURNS TRIGGER AS $$\n",
        "BEGIN\n",
        "  PERFORM pg_notify('sync_entity_change', json_build_object(\n",
        "    'schema', current_schema(),\n",
        "    'source_category', NEW.source_category,\n",
        "    'source_id', NEW.source_id,\n",
        "    'latest_skiptoken', NEW.latest_skiptoken,\n",
        "    'deleted', NEW.deleted\n",
        "  )::text);\n",
        "  RETURN NEW;\n",
        "END;\n",
        "$$ LANGUAGE plpgsql;\n\n",
        "DROP TRIGGER IF EXISTS sync_entity_change_trigger ON sync_entity;\n",
        "CREATE TRIGGER sync_entity_change_trigger\n",
        "AFTER INSERT OR UPDATE ON sync_entity\n",
        "FOR EACH ROW EXECUTE FUNCTION notify_sync_entity_change();\n",
    ));
}

fn sync_category_table() -> TableSpec {
    TableSpec {
        name: "sync_category".to_owned(),
        kind: TableKind::SyncMetadata,
        columns: columns(&[
            ("source_category", SqlType::Text, false),
            ("latest_skiptoken", SqlType::BigInteger, false),
            ("last_synced_at", SqlType::TimestampTz, true),
            ("next_url", SqlType::Text, true),
            ("resume_url", SqlType::Text, true),
            ("state", SqlType::Text, false),
            ("last_fetch_at", SqlType::TimestampTz, true),
            ("caught_up_at", SqlType::TimestampTz, true),
        ]),
        primary_key: names(&["source_category"]),
        foreign_keys: Vec::new(),
        unique_constraints: Vec::new(),
        check_constraints: Vec::new(),
    }
}

fn ingest_error_table() -> TableSpec {
    TableSpec {
        name: "ingest_error".to_owned(),
        kind: TableKind::SyncMetadata,
        columns: columns(&[
            ("id", SqlType::BigIdentity, false),
            ("phase", SqlType::Text, false),
            ("source_category", SqlType::Text, false),
            ("source_id", SqlType::Uuid, true),
            ("latest_skiptoken", SqlType::BigInteger, true),
            ("message", SqlType::Text, false),
            ("payload", SqlType::Jsonb, true),
            ("created_at", SqlType::TimestampTz, false),
        ]),
        primary_key: names(&["id"]),
        foreign_keys: Vec::new(),
        unique_constraints: Vec::new(),
        check_constraints: Vec::new(),
    }
}

fn search_index_cursor_table() -> TableSpec {
    TableSpec {
        name: "search_index_cursor".to_owned(),
        kind: TableKind::SearchIndexCursor,
        columns: columns(&[
            ("index_name", SqlType::Text, false),
            ("source_category", SqlType::Text, false),
            ("latest_skiptoken", SqlType::BigInteger, false),
            ("last_indexed_at", SqlType::TimestampTz, true),
            ("state", SqlType::Text, false),
            ("last_error", SqlType::Text, true),
        ]),
        primary_key: names(&["index_name", "source_category"]),
        foreign_keys: Vec::new(),
        unique_constraints: Vec::new(),
        check_constraints: Vec::new(),
    }
}

fn search_index_failure_table() -> TableSpec {
    TableSpec {
        name: "search_index_failure".to_owned(),
        kind: TableKind::SearchIndexFailure,
        columns: columns(&[
            ("id", SqlType::BigIdentity, false),
            ("index_name", SqlType::Text, false),
            ("source_category", SqlType::Text, false),
            ("source_id", SqlType::Uuid, false),
            ("latest_skiptoken", SqlType::BigInteger, false),
            ("operation", SqlType::Text, false),
            ("attempt_count", SqlType::Integer, false),
            ("next_retry_at", SqlType::TimestampTz, false),
            ("error", SqlType::Text, false),
            ("created_at", SqlType::TimestampTz, false),
            ("updated_at", SqlType::TimestampTz, false),
        ]),
        primary_key: names(&["id"]),
        foreign_keys: Vec::new(),
        unique_constraints: vec![UniqueConstraintSpec {
            columns: names(&[
                "index_name",
                "source_category",
                "source_id",
                "latest_skiptoken",
                "operation",
            ]),
        }],
        check_constraints: Vec::new(),
    }
}

fn sync_entity_table() -> TableSpec {
    TableSpec {
        name: "sync_entity".to_owned(),
        kind: TableKind::EntityRegistry,
        columns: metadata_columns(),
        primary_key: identity_columns(),
        foreign_keys: Vec::new(),
        unique_constraints: Vec::new(),
        check_constraints: Vec::new(),
    }
}

fn entity_table(entity: &'static EntityType) -> TableSpec {
    let mut columns = metadata_columns();

    for base_attribute in entity.base_attributes {
        match base_attribute.name {
            "id" | "verwijderd" | "bijgewerkt" => {}
            _ => columns.push(ColumnSpec {
                name: sql_name(base_attribute.name),
                sql_type: sql_type(base_attribute.xsd_type),
                nullable: !base_attribute.required,
            }),
        }
    }
    if entity.base == "downloadEntiteitType" {
        columns.push(ColumnSpec {
            name: "enclosure_url".to_owned(),
            sql_type: SqlType::Text,
            nullable: true,
        });
    }

    for field in entity
        .fields
        .iter()
        .filter(|field| field.kind == FieldKind::Attribute && !is_repeated(field.max_occurs))
    {
        columns.push(ColumnSpec {
            name: sql_name(field.name),
            sql_type: sql_type(field.xsd_type),
            nullable: field.min_occurs == 0 || field.nillable,
        });
    }

    TableSpec {
        name: sql_name(entity.category),
        kind: TableKind::Entity {
            category: entity.category,
        },
        columns,
        primary_key: identity_columns(),
        foreign_keys: vec![ForeignKeySpec {
            columns: identity_columns(),
            referenced_table: "sync_entity".to_owned(),
            referenced_columns: identity_columns(),
            on_delete: ForeignKeyAction::Cascade,
        }],
        unique_constraints: Vec::new(),
        check_constraints: Vec::new(),
    }
}

fn repeated_scalar_table(entity: &'static EntityType, field: &'static Field) -> TableSpec {
    TableSpec {
        name: format!("{}__{}", sql_name(entity.category), sql_name(field.name)),
        kind: TableKind::RepeatedScalar {
            category: entity.category,
            field_name: field.name,
        },
        columns: columns(&[
            ("source_category", SqlType::Text, false),
            ("source_id", SqlType::Uuid, false),
            ("ordinal", SqlType::Integer, false),
            ("value", sql_type(field.xsd_type), field.nillable),
        ]),
        primary_key: names(&["source_category", "source_id", "ordinal"]),
        foreign_keys: vec![ForeignKeySpec {
            columns: identity_columns(),
            referenced_table: sql_name(entity.category),
            referenced_columns: identity_columns(),
            on_delete: ForeignKeyAction::Cascade,
        }],
        unique_constraints: Vec::new(),
        check_constraints: Vec::new(),
    }
}

fn relation_table(entity: &'static EntityType, field: &'static Field) -> TableSpec {
    let mut unique_constraints = Vec::new();
    if field.max_occurs == Occurs::Exactly(1) {
        unique_constraints.push(UniqueConstraintSpec {
            columns: names(&["source_category", "source_id", "relation_name"]),
        });
    }

    TableSpec {
        name: format!("{}__{}", sql_name(entity.category), sql_name(field.name)),
        kind: TableKind::Relation {
            source_category: entity.category,
            relation_name: field.name,
        },
        columns: columns(&[
            ("source_category", SqlType::Text, false),
            ("source_id", SqlType::Uuid, false),
            ("relation_name", SqlType::Text, false),
            ("target_category", SqlType::Text, false),
            ("target_id", SqlType::Uuid, false),
            ("ordinal", SqlType::Integer, false),
            ("source_updated_at", SqlType::TimestampTz, false),
        ]),
        primary_key: names(&[
            "source_category",
            "source_id",
            "relation_name",
            "target_category",
            "target_id",
            "ordinal",
        ]),
        foreign_keys: vec![
            ForeignKeySpec {
                columns: identity_columns(),
                referenced_table: sql_name(entity.category),
                referenced_columns: identity_columns(),
                on_delete: ForeignKeyAction::Cascade,
            },
            ForeignKeySpec {
                columns: names(&["target_category", "target_id"]),
                referenced_table: "sync_entity".to_owned(),
                referenced_columns: identity_columns(),
                on_delete: ForeignKeyAction::Restrict,
            },
        ],
        unique_constraints,
        check_constraints: Vec::new(),
    }
}

fn document_asset_table() -> TableSpec {
    TableSpec {
        name: "document_asset".to_owned(),
        kind: TableKind::DocumentAsset,
        columns: columns(&[
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
        ]),
        primary_key: names(&["id"]),
        foreign_keys: vec![ForeignKeySpec {
            columns: names(&["document_source_category", "document_source_id"]),
            referenced_table: "document".to_owned(),
            referenced_columns: identity_columns(),
            on_delete: ForeignKeyAction::Cascade,
        }],
        unique_constraints: vec![UniqueConstraintSpec {
            columns: names(&[
                "document_source_category",
                "document_source_id",
                "asset_url",
            ]),
        }],
        check_constraints: vec![CheckConstraintSpec {
            name: "document_asset_retrieval_status_check".to_owned(),
            expression: "retrieval_status IN ('pending', 'fetched', 'not_found', 'unsupported_content_type', 'failed')"
                .to_owned(),
        }],
    }
}

fn document_content_table() -> TableSpec {
    TableSpec {
        name: "document_content".to_owned(),
        kind: TableKind::DocumentContent,
        columns: columns(&[
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
        ]),
        primary_key: names(&["id"]),
        foreign_keys: vec![
            ForeignKeySpec {
                columns: names(&["document_asset_id"]),
                referenced_table: "document_asset".to_owned(),
                referenced_columns: names(&["id"]),
                on_delete: ForeignKeyAction::Cascade,
            },
            ForeignKeySpec {
                columns: names(&["document_source_category", "document_source_id"]),
                referenced_table: "document".to_owned(),
                referenced_columns: identity_columns(),
                on_delete: ForeignKeyAction::Cascade,
            },
        ],
        unique_constraints: vec![UniqueConstraintSpec {
            columns: names(&["document_asset_id", "source_hash", "output_hash"]),
        }],
        check_constraints: vec![
            CheckConstraintSpec {
                name: "document_content_extraction_status_check".to_owned(),
                expression: "extraction_status IN ('pending', 'extracted', 'empty', 'failed')"
                    .to_owned(),
            },
            CheckConstraintSpec {
                name: "document_content_validation_status_check".to_owned(),
                expression: "validation_status IN ('unverified', 'valid', 'invalid')".to_owned(),
            },
            CheckConstraintSpec {
                name: "document_content_extracted_body_check".to_owned(),
                expression: "(extraction_status = 'failed' AND extraction_error IS NOT NULL) OR (extraction_status <> 'failed' AND (extracted_text IS NOT NULL OR extracted_html IS NOT NULL))".to_owned(),
            },
        ],
    }
}

fn entity_indexes(entity: &EntityType, table: &TableSpec) -> Vec<IndexSpec> {
    let mut indexes = vec![
        index(
            &table.name,
            &["source_id"],
            false,
            IndexPurpose::PrimaryUuidLookup,
        ),
        index(
            &table.name,
            &["source_category", "source_updated_at"],
            false,
            IndexPurpose::EntityUpdatedAt,
        ),
        index(
            &table.name,
            &["source_category", "source_id"],
            false,
            IndexPurpose::ForeignKeyPath,
        ),
    ];

    if entity.category == "Document" {
        indexes.push(index(
            &table.name,
            &["document_nummer"],
            false,
            IndexPurpose::DocumentNumber,
        ));
    }

    if entity.base == "downloadEntiteitType" {
        indexes.push(index(
            &table.name,
            &["source_category", "source_id", "content_type"],
            false,
            IndexPurpose::AssetOwner,
        ));
    }

    let mut date_columns = Vec::new();
    for column in &table.columns {
        if matches!(column.sql_type, SqlType::Date | SqlType::TimestampTz) {
            date_columns.push(column.name.as_str());
        }
    }
    date_columns.sort_unstable();
    for date_column in date_columns {
        indexes.push(index(
            &table.name,
            &[date_column],
            false,
            IndexPurpose::DateScan,
        ));
    }

    indexes
}

fn metadata_columns() -> Vec<ColumnSpec> {
    columns(&[
        ("source_category", SqlType::Text, false),
        ("source_id", SqlType::Uuid, false),
        ("latest_skiptoken", SqlType::BigInteger, false),
        ("deleted", SqlType::Boolean, false),
        ("source_updated_at", SqlType::TimestampTz, false),
        ("atom_updated_at", SqlType::TimestampTz, false),
    ])
}

fn sql_type(xsd_type: &str) -> SqlType {
    match xsd_type {
        "idType" | "referentieLiteral" | "referentieType" => SqlType::Uuid,
        "xs:boolean" | "booleanType" => SqlType::Boolean,
        "xs:int" | "xs:unsignedInt" | "intType" => SqlType::Integer,
        "xs:long" => SqlType::BigInteger,
        "xs:dateTime" => SqlType::TimestampTz,
        "xs:date" => SqlType::Date,
        _ => SqlType::Text,
    }
}

fn is_repeated(occurs: Occurs) -> bool {
    !matches!(occurs, Occurs::Exactly(1))
}

fn columns(specs: &[(&str, SqlType, bool)]) -> Vec<ColumnSpec> {
    specs
        .iter()
        .map(|(name, sql_type, nullable)| ColumnSpec {
            name: (*name).to_owned(),
            sql_type: *sql_type,
            nullable: *nullable,
        })
        .collect()
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn identity_columns() -> Vec<String> {
    names(&["source_category", "source_id"])
}

fn index(table_name: &str, columns: &[&str], unique: bool, purpose: IndexPurpose) -> IndexSpec {
    let purpose_name = format!("{purpose:?}").to_ascii_lowercase();
    let column_part = columns.join("_");
    let raw_name = format!("idx_{table_name}_{purpose_name}_{column_part}");

    IndexSpec {
        name: shorten_identifier(&raw_name),
        table_name: table_name.to_owned(),
        columns: names(columns),
        unique,
        purpose,
    }
}

fn render_create_table_if_not_exists(sql: &mut String, table: &TableSpec) {
    writeln!(sql, "CREATE TABLE IF NOT EXISTS {} (", ident(&table.name))
        .expect("writing to String cannot fail");
    let mut clauses = Vec::new();

    for column in &table.columns {
        clauses.push(format!(
            "    {} {}{}",
            ident(&column.name),
            column.sql_type.sql(),
            if column.nullable { "" } else { " NOT NULL" }
        ));
    }

    clauses.push(format!(
        "    PRIMARY KEY ({})",
        ident_list(&table.primary_key)
    ));

    for unique_constraint in &table.unique_constraints {
        clauses.push(format!(
            "    UNIQUE ({})",
            ident_list(&unique_constraint.columns)
        ));
    }

    for check_constraint in &table.check_constraints {
        clauses.push(format!(
            "    CONSTRAINT {} CHECK ({})",
            ident(&check_constraint.name),
            check_constraint.expression
        ));
    }

    for foreign_key in &table.foreign_keys {
        let on_delete = match foreign_key.on_delete {
            ForeignKeyAction::Cascade => "CASCADE",
            ForeignKeyAction::Restrict => "RESTRICT",
        };
        clauses.push(format!(
            "    FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {}",
            ident_list(&foreign_key.columns),
            ident(&foreign_key.referenced_table),
            ident_list(&foreign_key.referenced_columns),
            on_delete
        ));
    }

    writeln!(sql, "{}", clauses.join(",\n")).expect("writing to String cannot fail");
    sql.push_str(");\n");
}

fn render_create_index_if_not_exists(sql: &mut String, index_spec: &IndexSpec) {
    let unique = if index_spec.unique { "UNIQUE " } else { "" };
    writeln!(
        sql,
        "CREATE {unique}INDEX IF NOT EXISTS {} ON {} ({});",
        ident(&index_spec.name),
        ident(&index_spec.table_name),
        ident_list(&index_spec.columns)
    )
    .expect("writing to String cannot fail");
}

impl SqlType {
    fn sql(self) -> &'static str {
        match self {
            Self::Uuid => "uuid",
            Self::Text => "text",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::BigInteger => "bigint",
            Self::BigIdentity => "bigint GENERATED BY DEFAULT AS IDENTITY",
            Self::TimestampTz => "timestamptz",
            Self::Date => "date",
            Self::Jsonb => "jsonb",
        }
    }
}

fn ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn ident_list(identifiers: &[String]) -> String {
    identifiers
        .iter()
        .map(|identifier| ident(identifier))
        .collect::<Vec<_>>()
        .join(", ")
}

fn shorten_identifier(identifier: &str) -> String {
    const MAX_IDENTIFIER_LEN: usize = 63;

    if identifier.len() <= MAX_IDENTIFIER_LEN {
        return identifier.to_owned();
    }

    let hash = stable_hash(identifier);
    let suffix = format!("_{hash:016x}");
    let keep = MAX_IDENTIFIER_LEN - suffix.len();
    format!("{}{}", &identifier[..keep], suffix)
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = StableHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
struct StableHasher(u64);

impl Hasher for StableHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        };
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.0 = hash;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
