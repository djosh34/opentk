use std::{collections::BTreeSet, fs, path::Path};

use opentk_core::official_schema::{self, BaseAttribute, EntityType, Field, FieldKind, Occurs};
use thiserror::Error;

const XSD_PREFIX: &str = "tkData-v1-0-";
const XSD_SUFFIX: &str = ".xsd";
const NON_ENTITY_XSDS: &[&str] = &[
    "tkData-v1-0-base.xsd",
    "tkData-v1-0-identiteit.xsd",
    "tkData-v1-0-iso-8601.xsd",
    "tkData-v1-0-resource.xsd",
];
const RELATION_TYPES: &[&str] = &[
    "referentieLiteral",
    "referentieBijgewerktLiteral",
    "referentieType",
    "relatieType",
];

pub const OFFICIAL_REPOSITORY: &str =
    "https://github.com/TweedeKamerDerStaten-Generaal/OpenDataPortaal";
pub const PINNED_COMMIT: &str = "f2439f4a5b8bafa116245420e295d01c879d6b5f";

pub const TASK_DOCUMENTED_SOURCE_GAPS: &[SourceGap] = &[
    SourceGap {
        category: "DocumentPublicatie",
        reason: "task/informatiemodel category has no standalone XSD in the pinned official tree",
    },
    SourceGap {
        category: "DocumentPublicatieMetadata",
        reason: "task/informatiemodel category has no standalone XSD in the pinned official tree",
    },
    SourceGap {
        category: "FractieAanvullendGegeven",
        reason: "informatiemodel category has no standalone XSD in the pinned official tree",
    },
];

pub const TASK_DOCUMENTED_LIVE_FIELD_DRIFTS: &[LiveFieldDrift] = &[
    LiveFieldDrift {
        category: "Toezegging",
        field_name: "kamerbriefNakoming",
        kind: FieldKind::Attribute,
        min_occurs: 0,
        max_occurs: Occurs::Unbounded,
        nillable: true,
        xsd_type: "stringType",
        order: 20,
        reason: "live SyncFeed payload emitted this field more than once during full sync run 20260427-022641 although the pinned official XSD says maxOccurs=1",
    },
    LiveFieldDrift {
        category: "Toezegging",
        field_name: "toegezegdAan",
        kind: FieldKind::Relation,
        min_occurs: 0,
        max_occurs: Occurs::Unbounded,
        nillable: true,
        xsd_type: "referentieLiteral",
        order: 23,
        reason: "live SyncFeed payload exposes this relation more than once although the pinned official XSD only lists toegezegdAanFractie and toegezegdAanPersoon",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceGap {
    pub category: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveFieldDrift {
    pub category: &'static str,
    pub field_name: &'static str,
    pub kind: FieldKind,
    pub min_occurs: u32,
    pub max_occurs: Occurs,
    pub nillable: bool,
    pub xsd_type: &'static str,
    pub order: u32,
    pub reason: &'static str,
}

#[derive(Debug, Error)]
pub enum SchemaSourceError {
    #[error("failed to read official XSD directory {path}: {source}")]
    ReadDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to read official XSD file {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse official XSD file {path}: {source}")]
    ParseXml {
        path: String,
        source: roxmltree::Error,
    },
    #[error("official XSD file {path} has no top-level entity element")]
    MissingEntityElement { path: String },
    #[error("official XSD file {path} has no complexType extension")]
    MissingExtension { path: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedEntity {
    pub category: String,
    pub xml_element: String,
    pub rust_name: String,
    pub base: String,
    pub base_attributes: Vec<ExtractedBaseAttribute>,
    pub fields: Vec<ExtractedField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedBaseAttribute {
    pub name: String,
    pub xsd_type: String,
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedField {
    pub name: String,
    pub kind: FieldKind,
    pub min_occurs: u32,
    pub max_occurs: Occurs,
    pub nillable: bool,
    pub xsd_type: String,
    pub order: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaMismatch {
    pub category: String,
    pub detail: String,
}

#[must_use]
pub fn official_xsd_dir() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/official_sources/tweedekamer/xsd"
    ))
}

/// Extracts entity metadata from a directory of official Tweede Kamer XSD files.
///
/// # Errors
///
/// Returns an error when the directory or a file cannot be read, when XML is
/// malformed, or when an entity XSD does not contain the expected entity
/// element and type extension.
pub fn extract_entities_from_dir(path: &Path) -> Result<Vec<ExtractedEntity>, SchemaSourceError> {
    let mut files = fs::read_dir(path)
        .map_err(|source| SchemaSourceError::ReadDir {
            path: path.display().to_string(),
            source,
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| SchemaSourceError::ReadDir {
            path: path.display().to_string(),
            source,
        })?;

    files.sort();

    files
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_entity_xsd)
        })
        .map(|path| extract_entity_from_file(&path))
        .collect()
}

#[must_use]
pub fn compare_model_to_sources(sources: &[ExtractedEntity]) -> Vec<SchemaMismatch> {
    let mut mismatches = Vec::new();
    let source_categories = sources
        .iter()
        .map(|entity| entity.category.as_str())
        .collect::<BTreeSet<_>>();
    let model_categories = official_schema::entity_types()
        .iter()
        .map(|entity| entity.category)
        .collect::<BTreeSet<_>>();

    for missing in source_categories.difference(&model_categories) {
        mismatches.push(SchemaMismatch {
            category: (*missing).to_owned(),
            detail: "official XSD entity is missing from opentk-core model".to_owned(),
        });
    }

    for extra in model_categories.difference(&source_categories) {
        mismatches.push(SchemaMismatch {
            category: (*extra).to_owned(),
            detail: "opentk-core model entity has no vendored official XSD".to_owned(),
        });
    }

    for source in sources {
        let Some(model) = official_schema::entity_named(&source.category) else {
            continue;
        };
        compare_entity(model, source, &mut mismatches);
    }

    mismatches
}

/// Verifies the checked-in core schema model against the vendored XSD files.
///
/// # Errors
///
/// Returns an error when the vendored XSD directory cannot be read or parsed.
pub fn verify_checked_in_model() -> Result<Vec<SchemaMismatch>, SchemaSourceError> {
    verify_model_against_dir(official_xsd_dir())
}

/// Verifies the checked-in core schema model against XSD files in `path`.
///
/// # Errors
///
/// Returns an error when the supplied XSD directory cannot be read or parsed.
pub fn verify_model_against_dir(path: &Path) -> Result<Vec<SchemaMismatch>, SchemaSourceError> {
    let sources = extract_entities_from_dir(path)?;
    Ok(compare_model_to_sources(&sources))
}

#[must_use]
pub fn source_gap_report() -> String {
    let mut lines = TASK_DOCUMENTED_SOURCE_GAPS
        .iter()
        .map(|gap| format!("{}: {}", gap.category, gap.reason))
        .collect::<Vec<_>>();

    lines.extend(
        TASK_DOCUMENTED_LIVE_FIELD_DRIFTS
            .iter()
            .map(|drift| format!("{}.{}: {}", drift.category, drift.field_name, drift.reason)),
    );

    lines.join("\n")
}

fn is_entity_xsd(file_name: &str) -> bool {
    file_name.starts_with(XSD_PREFIX)
        && file_name.ends_with(XSD_SUFFIX)
        && !NON_ENTITY_XSDS.contains(&file_name)
}

fn extract_entity_from_file(path: &Path) -> Result<ExtractedEntity, SchemaSourceError> {
    let path_string = path.display().to_string();
    let xml = fs::read_to_string(path).map_err(|source| SchemaSourceError::ReadFile {
        path: path_string.clone(),
        source,
    })?;
    let document =
        roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')).map_err(|source| {
            SchemaSourceError::ParseXml {
                path: path_string.clone(),
                source,
            }
        })?;

    let top_element = document
        .root_element()
        .children()
        .find(|node| node.has_tag_name((XML_SCHEMA_NS, "element")))
        .ok_or_else(|| SchemaSourceError::MissingEntityElement {
            path: path_string.clone(),
        })?;
    let xml_element =
        top_element
            .attribute("name")
            .ok_or_else(|| SchemaSourceError::MissingEntityElement {
                path: path_string.clone(),
            })?;
    let extension = document
        .descendants()
        .find(|node| node.has_tag_name((XML_SCHEMA_NS, "extension")))
        .ok_or_else(|| SchemaSourceError::MissingExtension {
            path: path_string.clone(),
        })?;
    let base = extension
        .attribute("base")
        .ok_or(SchemaSourceError::MissingExtension { path: path_string })?;
    let fields = extension
        .children()
        .find(|node| node.has_tag_name((XML_SCHEMA_NS, "sequence")))
        .into_iter()
        .flat_map(|sequence| {
            sequence
                .children()
                .filter(|node| node.has_tag_name((XML_SCHEMA_NS, "element")))
        })
        .enumerate()
        .map(|(index, element)| extract_field(index, element))
        .collect();

    Ok(ExtractedEntity {
        category: category_from_xml_element(xml_element),
        xml_element: xml_element.to_owned(),
        rust_name: category_from_xml_element(xml_element),
        base: base.to_owned(),
        base_attributes: base_attributes(base),
        fields,
    })
}

fn extract_field(index: usize, element: roxmltree::Node<'_, '_>) -> ExtractedField {
    let xsd_type = element.attribute("type").unwrap_or_default();

    ExtractedField {
        name: element.attribute("name").unwrap_or_default().to_owned(),
        kind: if RELATION_TYPES.contains(&xsd_type) {
            FieldKind::Relation
        } else {
            FieldKind::Attribute
        },
        min_occurs: element
            .attribute("minOccurs")
            .unwrap_or("1")
            .parse()
            .expect("official XSD minOccurs is numeric"),
        max_occurs: match element.attribute("maxOccurs").unwrap_or("1") {
            "unbounded" => Occurs::Unbounded,
            value => Occurs::Exactly(value.parse().expect("official XSD maxOccurs is numeric")),
        },
        nillable: element.attribute("nillable") == Some("true"),
        xsd_type: xsd_type.to_owned(),
        order: u32::try_from(index + 1).expect("field count fits in u32"),
    }
}

fn compare_entity(
    model: &'static EntityType,
    source: &ExtractedEntity,
    mismatches: &mut Vec<SchemaMismatch>,
) {
    if model.xml_element != source.xml_element {
        push_mismatch(
            model.category,
            "xml element",
            model.xml_element,
            &source.xml_element,
            mismatches,
        );
    }
    if model.base != source.base {
        push_mismatch(model.category, "base", model.base, &source.base, mismatches);
    }
    compare_base_attributes(model, source, mismatches);
    compare_fields(model, source, mismatches);
}

fn compare_base_attributes(
    model: &'static EntityType,
    source: &ExtractedEntity,
    mismatches: &mut Vec<SchemaMismatch>,
) {
    let model_attributes = model
        .base_attributes
        .iter()
        .map(base_attribute_key)
        .collect::<Vec<_>>();
    let source_attributes = source
        .base_attributes
        .iter()
        .map(extracted_base_attribute_key)
        .collect::<Vec<_>>();

    if model_attributes != source_attributes {
        mismatches.push(SchemaMismatch {
            category: model.category.to_owned(),
            detail: format!(
                "base attributes differ: model={model_attributes:?} source={source_attributes:?}"
            ),
        });
    }
}

fn compare_fields(
    model: &'static EntityType,
    source: &ExtractedEntity,
    mismatches: &mut Vec<SchemaMismatch>,
) {
    let model_fields = model.fields.iter().map(field_key).collect::<Vec<_>>();
    let source_fields = source_fields_with_documented_live_drifts(source);

    if model_fields != source_fields {
        mismatches.push(SchemaMismatch {
            category: model.category.to_owned(),
            detail: format!("fields differ: model={model_fields:?} source={source_fields:?}"),
        });
    }
}

fn source_fields_with_documented_live_drifts(
    source: &ExtractedEntity,
) -> Vec<(&str, FieldKind, u32, Occurs, bool, &str, u32)> {
    let mut source_fields = source
        .fields
        .iter()
        .map(extracted_field_key)
        .collect::<Vec<_>>();

    for drift in TASK_DOCUMENTED_LIVE_FIELD_DRIFTS
        .iter()
        .filter(|drift| drift.category == source.category)
    {
        let drift_key = live_field_drift_key(drift);
        if let Some(existing) = source_fields
            .iter_mut()
            .find(|field| field.0 == drift.field_name)
        {
            *existing = drift_key;
        } else {
            source_fields.push(drift_key);
        }
    }

    source_fields
}

fn push_mismatch(
    category: &str,
    label: &str,
    model: &str,
    source: &str,
    mismatches: &mut Vec<SchemaMismatch>,
) {
    mismatches.push(SchemaMismatch {
        category: category.to_owned(),
        detail: format!("{label} differs: model={model:?} source={source:?}"),
    });
}

fn field_key(field: &Field) -> (&str, FieldKind, u32, Occurs, bool, &str, u32) {
    (
        field.name,
        field.kind,
        field.min_occurs,
        field.max_occurs,
        field.nillable,
        field.xsd_type,
        field.order,
    )
}

fn extracted_field_key(field: &ExtractedField) -> (&str, FieldKind, u32, Occurs, bool, &str, u32) {
    (
        &field.name,
        field.kind,
        field.min_occurs,
        field.max_occurs,
        field.nillable,
        &field.xsd_type,
        field.order,
    )
}

fn live_field_drift_key(field: &LiveFieldDrift) -> (&str, FieldKind, u32, Occurs, bool, &str, u32) {
    (
        field.field_name,
        field.kind,
        field.min_occurs,
        field.max_occurs,
        field.nillable,
        field.xsd_type,
        field.order,
    )
}

fn base_attribute_key(attribute: &BaseAttribute) -> (&str, &str, bool) {
    (attribute.name, attribute.xsd_type, attribute.required)
}

fn extracted_base_attribute_key(attribute: &ExtractedBaseAttribute) -> (&str, &str, bool) {
    (&attribute.name, &attribute.xsd_type, attribute.required)
}

fn base_attributes(base: &str) -> Vec<ExtractedBaseAttribute> {
    let mut attributes = vec![
        ExtractedBaseAttribute {
            name: "id".to_owned(),
            xsd_type: "idType".to_owned(),
            required: true,
        },
        ExtractedBaseAttribute {
            name: "verwijderd".to_owned(),
            xsd_type: "xs:boolean".to_owned(),
            required: true,
        },
        ExtractedBaseAttribute {
            name: "bijgewerkt".to_owned(),
            xsd_type: "xs:dateTime".to_owned(),
            required: true,
        },
    ];

    if base == "downloadEntiteitType" {
        attributes.extend([
            ExtractedBaseAttribute {
                name: "contentType".to_owned(),
                xsd_type: "xs:string".to_owned(),
                required: false,
            },
            ExtractedBaseAttribute {
                name: "contentLength".to_owned(),
                xsd_type: "xs:int".to_owned(),
                required: false,
            },
        ]);
    }

    attributes
}

fn category_from_xml_element(xml_element: &str) -> String {
    let mut chars = xml_element.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    first.to_uppercase().chain(chars).collect()
}

const XML_SCHEMA_NS: &str = "http://www.w3.org/2001/XMLSchema";
