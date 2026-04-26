use std::{collections::HashMap, fmt};

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use opentk_core::official_schema::{self, EntityType, Field, FieldKind, Occurs};
use quick_xml::{
    events::{BytesStart, Event},
    name::QName,
    Reader,
};
use reqwest::Url;
use thiserror::Error;
use uuid::Uuid;

use crate::syncfeed::SyncFeedEntry;

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedEntity {
    pub category: String,
    pub xml_element: String,
    pub source_id: Uuid,
    pub deleted: bool,
    pub source_updated_at: DateTime<Utc>,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
    pub scalars: Vec<ParsedScalar>,
    pub relations: Vec<ParsedRelation>,
    pub enclosure_url: Option<Url>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedScalar {
    pub name: String,
    pub value: ParsedValue,
    pub ordinal: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedRelation {
    pub name: String,
    pub target_category: String,
    pub target_id: Uuid,
    pub target_updated_at: Option<DateTime<Utc>>,
    pub ordinal: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParsedValue {
    Text(String),
    Bool(bool),
    I32(i32),
    I64(i64),
    Date(NaiveDate),
    DateTime(DateTime<Utc>),
}

#[derive(Debug, Error)]
pub enum PayloadParseError {
    #[error("unknown category {category}")]
    UnknownCategory { category: String },
    #[error("entry for category {category} has no embedded XML payload")]
    MissingContent { category: String },
    #[error("category {category} XML root must be {expected}, got {actual}")]
    RootMismatch {
        category: String,
        expected: String,
        actual: String,
    },
    #[error("{category} payload is missing required attribute {attribute}")]
    MissingAttribute { category: String, attribute: String },
    #[error("{category}.{field} has invalid {datatype} value {value:?}: {message}")]
    InvalidValue {
        category: String,
        field: String,
        datatype: String,
        value: String,
        message: String,
    },
    #[error("{category}.{field} appeared more than once but is single-occurrence")]
    DuplicateSingleField { category: String, field: String },
    #[error("{category}.{field} is unknown")]
    UnknownField { category: String, field: String },
    #[error("{category}.{field} is nil but is not nullable")]
    NonNullableNil { category: String, field: String },
    #[error("{category}.{field} relation is missing ref attribute")]
    MissingRelationRef { category: String, field: String },
    #[error("invalid XML for {category}: {message}")]
    Xml { category: String, message: String },
}

/// Parse the embedded XML payload from a fetched Atom entry.
///
/// # Errors
///
/// Returns an error when the entry has no embedded content, the category is not
/// modeled, or the XML payload violates the strict entity parser rules.
pub fn parse_entry_payload(entry: &SyncFeedEntry) -> Result<ParsedEntity, PayloadParseError> {
    let xml = entry
        .content_xml
        .as_deref()
        .ok_or_else(|| PayloadParseError::MissingContent {
            category: entry.category.clone(),
        })?;
    let mut entity = parse_entity_xml(&entry.category, xml)?;
    entity.enclosure_url.clone_from(&entry.enclosure_url);
    Ok(entity)
}

/// Parse one official entity XML payload into typed scalar and relation values.
///
/// # Errors
///
/// Returns an error for unknown categories, root/category mismatches, malformed
/// XML, missing required metadata, invalid typed values, unknown fields,
/// multiplicity violations, or invalid nil usage.
pub fn parse_entity_xml(category: &str, xml: &str) -> Result<ParsedEntity, PayloadParseError> {
    let entity = official_schema::entity_named(category).ok_or_else(|| {
        PayloadParseError::UnknownCategory {
            category: category.to_owned(),
        }
    })?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader
            .read_event()
            .map_err(|source| PayloadParseError::Xml {
                category: category.to_owned(),
                message: source.to_string(),
            })? {
            Event::Start(root) => return parse_root(entity, &root, &mut reader),
            Event::Empty(root) => return parse_empty_root(entity, &root),
            Event::Eof => {
                return Err(PayloadParseError::Xml {
                    category: category.to_owned(),
                    message: "missing root element".to_owned(),
                });
            }
            _ => {}
        }
    }
}

fn parse_empty_root(
    entity: &'static EntityType,
    root: &BytesStart<'_>,
) -> Result<ParsedEntity, PayloadParseError> {
    let local_name = local_name(root.name());
    if local_name != entity.xml_element {
        return Err(PayloadParseError::RootMismatch {
            category: entity.category.to_owned(),
            expected: entity.xml_element.to_owned(),
            actual: local_name,
        });
    }
    let parsed = parse_base_attributes(entity, root)?;
    if !parsed.deleted {
        validate_required_fields(entity, &HashMap::new())?;
    }
    Ok(parsed)
}

fn parse_root(
    entity: &'static EntityType,
    root: &BytesStart<'_>,
    reader: &mut Reader<&[u8]>,
) -> Result<ParsedEntity, PayloadParseError> {
    let root_name = local_name(root.name());
    if root_name != entity.xml_element {
        return Err(PayloadParseError::RootMismatch {
            category: entity.category.to_owned(),
            expected: entity.xml_element.to_owned(),
            actual: root_name,
        });
    }

    let mut parsed = parse_base_attributes(entity, root)?;
    if parsed.deleted {
        reader
            .read_to_end(root.name())
            .map_err(|source| PayloadParseError::Xml {
                category: entity.category.to_owned(),
                message: source.to_string(),
            })?;
        return Ok(parsed);
    }

    let mut counts: HashMap<String, i32> = HashMap::new();

    loop {
        match reader
            .read_event()
            .map_err(|source| PayloadParseError::Xml {
                category: entity.category.to_owned(),
                message: source.to_string(),
            })? {
            Event::Start(child) => {
                parse_child(entity, &mut parsed, &mut counts, &child, reader)?;
            }
            Event::Empty(child) => {
                parse_empty_child(entity, &mut parsed, &mut counts, &child)?;
            }
            Event::End(end) if local_name(end.name()) == entity.xml_element => break,
            Event::Text(text) if text.iter().all(u8::is_ascii_whitespace) => {}
            Event::Comment(_) | Event::CData(_) => {}
            Event::Eof => {
                return Err(PayloadParseError::Xml {
                    category: entity.category.to_owned(),
                    message: "unexpected end of XML".to_owned(),
                });
            }
            event => {
                return Err(PayloadParseError::Xml {
                    category: entity.category.to_owned(),
                    message: format!("unexpected XML event {event:?}"),
                });
            }
        }
    }

    validate_required_fields(entity, &counts)?;
    Ok(parsed)
}

fn parse_child(
    entity: &'static EntityType,
    parsed: &mut ParsedEntity,
    counts: &mut HashMap<String, i32>,
    child: &BytesStart<'_>,
    reader: &mut Reader<&[u8]>,
) -> Result<(), PayloadParseError> {
    let name = local_name(child.name());
    let field = entity
        .field_named(&name)
        .ok_or_else(|| PayloadParseError::UnknownField {
            category: entity.category.to_owned(),
            field: name.clone(),
        })?;
    let ordinal = next_ordinal(entity, field, counts)?;
    if is_nil(child)? {
        reader
            .read_to_end(child.name())
            .map_err(|source| PayloadParseError::Xml {
                category: entity.category.to_owned(),
                message: source.to_string(),
            })?;
        if field.nillable {
            return Ok(());
        }
        return Err(PayloadParseError::NonNullableNil {
            category: entity.category.to_owned(),
            field: field.name.to_owned(),
        });
    }

    match field.kind {
        FieldKind::Attribute => {
            let text = reader
                .read_text(child.name())
                .map_err(|source| PayloadParseError::Xml {
                    category: entity.category.to_owned(),
                    message: source.to_string(),
                })?;
            parsed.scalars.push(ParsedScalar {
                name: field.name.to_owned(),
                value: parse_value(entity.category, field.name, field.xsd_type, text.trim())?,
                ordinal,
            });
        }
        FieldKind::Relation => {
            let relation = parse_relation_or_absent(entity, field, child, ordinal)?;
            reader
                .read_to_end(child.name())
                .map_err(|source| PayloadParseError::Xml {
                    category: entity.category.to_owned(),
                    message: source.to_string(),
                })?;
            if let Some(relation) = relation {
                parsed.relations.push(relation);
            }
        }
    }
    Ok(())
}

fn parse_empty_child(
    entity: &'static EntityType,
    parsed: &mut ParsedEntity,
    counts: &mut HashMap<String, i32>,
    child: &BytesStart<'_>,
) -> Result<(), PayloadParseError> {
    let name = local_name(child.name());
    let field = entity
        .field_named(&name)
        .ok_or_else(|| PayloadParseError::UnknownField {
            category: entity.category.to_owned(),
            field: name.clone(),
        })?;
    let ordinal = next_ordinal(entity, field, counts)?;
    if is_nil(child)? {
        if field.nillable {
            return Ok(());
        }
        return Err(PayloadParseError::NonNullableNil {
            category: entity.category.to_owned(),
            field: field.name.to_owned(),
        });
    }

    match field.kind {
        FieldKind::Attribute => parsed.scalars.push(ParsedScalar {
            name: field.name.to_owned(),
            value: parse_value(entity.category, field.name, field.xsd_type, "")?,
            ordinal,
        }),
        FieldKind::Relation => {
            if let Some(relation) = parse_relation_or_absent(entity, field, child, ordinal)? {
                parsed.relations.push(relation);
            }
        }
    }
    Ok(())
}

fn parse_base_attributes(
    entity: &'static EntityType,
    root: &BytesStart<'_>,
) -> Result<ParsedEntity, PayloadParseError> {
    let attributes = attributes(root, entity.category)?;
    let source_id = parse_uuid_attr(entity.category, "id", &attributes)?;
    let deleted = parse_bool_attr(entity.category, "verwijderd", &attributes)?;
    let source_updated_at = parse_datetime_attr(entity.category, "bijgewerkt", &attributes)?;
    let content_type = attributes.get("contentType").cloned();
    let content_length = attributes
        .get("contentLength")
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|source| PayloadParseError::InvalidValue {
                    category: entity.category.to_owned(),
                    field: "contentLength".to_owned(),
                    datatype: "xs:int".to_owned(),
                    value: value.clone(),
                    message: source.to_string(),
                })
        })
        .transpose()?;

    for base_attribute in entity.base_attributes {
        if base_attribute.required && !attributes.contains_key(base_attribute.name) {
            return Err(PayloadParseError::MissingAttribute {
                category: entity.category.to_owned(),
                attribute: base_attribute.name.to_owned(),
            });
        }
    }

    Ok(ParsedEntity {
        category: entity.category.to_owned(),
        xml_element: entity.xml_element.to_owned(),
        source_id,
        deleted,
        source_updated_at,
        content_type,
        content_length,
        scalars: Vec::new(),
        relations: Vec::new(),
        enclosure_url: None,
    })
}

fn parse_relation(
    entity: &'static EntityType,
    field: &'static Field,
    child: &BytesStart<'_>,
    ordinal: i32,
) -> Result<ParsedRelation, PayloadParseError> {
    let attributes = attributes(child, entity.category)?;
    let target = attributes
        .get("ref")
        .ok_or_else(|| PayloadParseError::MissingRelationRef {
            category: entity.category.to_owned(),
            field: field.name.to_owned(),
        })?;
    let target_id = Uuid::parse_str(target).map_err(|source| PayloadParseError::InvalidValue {
        category: entity.category.to_owned(),
        field: field.name.to_owned(),
        datatype: field.xsd_type.to_owned(),
        value: target.clone(),
        message: source.to_string(),
    })?;
    let target_updated_at = attributes
        .get("bijgewerkt")
        .map(|value| parse_datetime(entity.category, field.name, value))
        .transpose()?;

    Ok(ParsedRelation {
        name: field.name.to_owned(),
        target_category: infer_target_category(field.name),
        target_id,
        target_updated_at,
        ordinal,
    })
}

fn parse_relation_or_absent(
    entity: &'static EntityType,
    field: &'static Field,
    child: &BytesStart<'_>,
    ordinal: i32,
) -> Result<Option<ParsedRelation>, PayloadParseError> {
    match parse_relation(entity, field, child, ordinal) {
        Ok(relation) => Ok(Some(relation)),
        Err(PayloadParseError::MissingRelationRef { .. }) if field.nillable => Ok(None),
        Err(error) => Err(error),
    }
}

fn parse_value(
    category: &str,
    field: &str,
    datatype: &str,
    value: &str,
) -> Result<ParsedValue, PayloadParseError> {
    match datatype {
        "xs:boolean" | "booleanType" => parse_bool(category, field, value).map(ParsedValue::Bool),
        "xs:int" | "xs:unsignedInt" | "intType" => value
            .parse::<i32>()
            .map(ParsedValue::I32)
            .map_err(|source| invalid_value(category, field, datatype, value, source)),
        "xs:long" => value
            .parse::<i64>()
            .map(ParsedValue::I64)
            .map_err(|source| invalid_value(category, field, datatype, value, source)),
        "xs:date" => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(ParsedValue::Date)
            .map_err(|source| invalid_value(category, field, datatype, value, source)),
        "xs:dateTime" => parse_datetime(category, field, value).map(ParsedValue::DateTime),
        _ => Ok(ParsedValue::Text(value.to_owned())),
    }
}

fn parse_uuid_attr(
    category: &str,
    field: &str,
    attributes: &HashMap<String, String>,
) -> Result<Uuid, PayloadParseError> {
    let value = required_attribute(category, field, attributes)?;
    Uuid::parse_str(value).map_err(|source| invalid_value(category, field, "idType", value, source))
}

fn parse_bool_attr(
    category: &str,
    field: &str,
    attributes: &HashMap<String, String>,
) -> Result<bool, PayloadParseError> {
    let value = required_attribute(category, field, attributes)?;
    parse_bool(category, field, value)
}

fn parse_datetime_attr(
    category: &str,
    field: &str,
    attributes: &HashMap<String, String>,
) -> Result<DateTime<Utc>, PayloadParseError> {
    let value = required_attribute(category, field, attributes)?;
    parse_datetime(category, field, value)
}

fn required_attribute<'a>(
    category: &str,
    field: &str,
    attributes: &'a HashMap<String, String>,
) -> Result<&'a str, PayloadParseError> {
    attributes
        .get(field)
        .map(String::as_str)
        .ok_or_else(|| PayloadParseError::MissingAttribute {
            category: category.to_owned(),
            attribute: field.to_owned(),
        })
}

fn parse_bool(category: &str, field: &str, value: &str) -> Result<bool, PayloadParseError> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(PayloadParseError::InvalidValue {
            category: category.to_owned(),
            field: field.to_owned(),
            datatype: "xs:boolean".to_owned(),
            value: value.to_owned(),
            message: "expected true, false, 1, or 0".to_owned(),
        }),
    }
}

fn parse_datetime(
    category: &str,
    field: &str,
    value: &str,
) -> Result<DateTime<Utc>, PayloadParseError> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(value) {
        return Ok(datetime.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .map(|datetime| datetime.and_utc())
        .map_err(|source| invalid_value(category, field, "xs:dateTime", value, source))
}

fn invalid_value(
    category: &str,
    field: &str,
    datatype: &str,
    value: &str,
    source: impl fmt::Display,
) -> PayloadParseError {
    PayloadParseError::InvalidValue {
        category: category.to_owned(),
        field: field.to_owned(),
        datatype: datatype.to_owned(),
        value: value.to_owned(),
        message: source.to_string(),
    }
}

fn next_ordinal(
    entity: &'static EntityType,
    field: &'static Field,
    counts: &mut HashMap<String, i32>,
) -> Result<i32, PayloadParseError> {
    let count = counts.entry(field.name.to_owned()).or_insert(0);
    *count += 1;
    if field.max_occurs == Occurs::Exactly(1) && *count > 1 {
        return Err(PayloadParseError::DuplicateSingleField {
            category: entity.category.to_owned(),
            field: field.name.to_owned(),
        });
    }
    Ok(*count)
}

fn validate_required_fields(
    entity: &'static EntityType,
    counts: &HashMap<String, i32>,
) -> Result<(), PayloadParseError> {
    for field in entity.fields {
        if field.min_occurs > 0 && !counts.contains_key(field.name) {
            return Err(PayloadParseError::Xml {
                category: entity.category.to_owned(),
                message: format!("missing required field {}", field.name),
            });
        }
    }
    Ok(())
}

fn attributes(
    start: &BytesStart<'_>,
    category: &str,
) -> Result<HashMap<String, String>, PayloadParseError> {
    let mut output = HashMap::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|source| PayloadParseError::Xml {
            category: category.to_owned(),
            message: source.to_string(),
        })?;
        output.insert(
            local_name(attribute.key),
            String::from_utf8(attribute.value.into_owned()).map_err(|source| {
                PayloadParseError::Xml {
                    category: category.to_owned(),
                    message: source.to_string(),
                }
            })?,
        );
    }
    Ok(output)
}

fn is_nil(start: &BytesStart<'_>) -> Result<bool, PayloadParseError> {
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|source| PayloadParseError::Xml {
            category: local_name(start.name()),
            message: source.to_string(),
        })?;
        if local_name(attribute.key) == "nil" {
            return Ok(matches!(attribute.value.as_ref(), b"true" | b"1"));
        }
    }
    Ok(false)
}

fn infer_target_category(field_name: &str) -> String {
    official_schema::entity_types()
        .iter()
        .find(|entity| field_matches_entity(field_name, entity))
        .map_or_else(
            || upper_camel(field_name),
            |entity| entity.category.to_owned(),
        )
}

fn field_matches_entity(field_name: &str, entity: &EntityType) -> bool {
    let normalized_field = normalize(field_name);
    normalized_field == normalize(entity.category)
        || normalized_field == normalize(entity.xml_element)
}

fn upper_camel(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

fn local_name(name: QName<'_>) -> String {
    let bytes = name.as_ref();
    let local = bytes
        .iter()
        .rposition(|byte| *byte == b':')
        .map_or(bytes, |position| &bytes[position + 1..]);
    String::from_utf8_lossy(local).into_owned()
}
