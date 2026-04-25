# SyncFeed Payload Parser

`opentk-sync::payload` parses embedded entity XML from `SyncFeed` entries. The
parser is strict and schema-driven: categories, root element names, fields,
relations, multiplicity, nullability, and datatype handling come from
`opentk-core::official_schema`.

Parser rules:

- `id`, `verwijderd`, and `bijgewerkt` are required base attributes and become
  `source_id`, `deleted`, and `source_updated_at`.
- `contentType` and `contentLength` are parsed for `downloadEntiteitType`
  entities, and `parse_entry_payload` also carries the Atom enclosure URL.
- Unknown categories, wrong root elements, unknown child elements, duplicate
  single-occurrence fields, invalid UUIDs, invalid booleans, invalid integers,
  invalid dates, and invalid datetimes are hard errors.
- `xsi:nil="true"` is accepted only for nillable fields and produces no current
  scalar value.
- Deleted entities carry only metadata. Scalar or relation body content on a
  delete marker is rejected so stale current rows cannot be recreated later.
- Relation literals read the `ref` attribute as a UUID. If a relation carries a
  `bijgewerkt` attribute, it is parsed as the target update timestamp.

Datatype mapping:

- `xs:boolean` and `booleanType` become `bool`.
- `xs:int`, `xs:unsignedInt`, and `intType` become `i32`.
- `xs:long` becomes `i64`.
- `xs:date` becomes `chrono::NaiveDate`.
- `xs:dateTime` becomes `chrono::DateTime<Utc>`.
- Other modeled scalar types remain text after XML validation of presence,
  multiplicity, and nil handling.
