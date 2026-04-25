//! Repository-owned metadata for the official Tweede Kamer `SyncFeed` model.

mod generated;

pub use generated::ENTITY_TYPES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityType {
    pub category: &'static str,
    pub xml_element: &'static str,
    pub rust_name: &'static str,
    pub base: &'static str,
    pub base_attributes: &'static [BaseAttribute],
    pub fields: &'static [Field],
}

impl EntityType {
    #[must_use]
    pub fn field_named(&self, name: &str) -> Option<&'static Field> {
        self.fields.iter().find(|field| field.name == name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseAttribute {
    pub name: &'static str,
    pub xsd_type: &'static str,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Field {
    pub name: &'static str,
    pub kind: FieldKind,
    pub min_occurs: u32,
    pub max_occurs: Occurs,
    pub nillable: bool,
    pub xsd_type: &'static str,
    pub order: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Attribute,
    Relation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Occurs {
    Exactly(u32),
    Unbounded,
}

#[must_use]
pub const fn entity_types() -> &'static [EntityType] {
    ENTITY_TYPES
}

#[must_use]
pub fn entity_named(name: &str) -> Option<&'static EntityType> {
    ENTITY_TYPES.iter().find(|entity| {
        entity.category == name || entity.xml_element == name || entity.rust_name == name
    })
}
