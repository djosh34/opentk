# Official SyncFeed Schema Model

`opentk-core::official_schema` is the repository-owned model of the Tweede
Kamer SyncFeed entity metadata. It captures entity categories, XML element
names, base entity attributes, scalar fields, relation fields, XSD datatypes,
field order, nullability, and multiplicity.

The vendored source XSDs live in:

```text
crates/opentk-sync/official_sources/tweedekamer/xsd/
```

They were copied from:

```text
https://github.com/TweedeKamerDerStaten-Generaal/OpenDataPortaal
commit f2439f4a5b8bafa116245420e295d01c879d6b5f
```

Default tests are offline. They compare the checked-in core model to the
checked-in official XSDs and fail if either side drifts.

## Refresh Procedure

Fetch the pinned upstream XSD tree into the vendored source directory:

```bash
rm -rf crates/opentk-sync/official_sources/tweedekamer/xsd
mkdir -p crates/opentk-sync/official_sources/tweedekamer/xsd
curl -fsSL \
  https://github.com/TweedeKamerDerStaten-Generaal/OpenDataPortaal/archive/f2439f4a5b8bafa116245420e295d01c879d6b5f.tar.gz \
  | tar -xz --strip-components=2 \
      -C crates/opentk-sync/official_sources/tweedekamer/xsd \
      OpenDataPortaal-f2439f4a5b8bafa116245420e295d01c879d6b5f/xsd
```

If the upstream commit changes, update `PINNED_COMMIT` in
`crates/opentk-sync/src/official_schema.rs`, refresh the XSDs, regenerate
`crates/opentk-core/src/official_schema/generated.rs` from the XSDs, and run:

```bash
cargo run -p opentk-sync --bin official-schema -- check
make check
make test
```

The check command exits non-zero and lists each mismatch when a source XSD and
the checked-in model disagree.

## Source Gaps

The task and public informatiemodel mention these categories, but the pinned
official XSD tree has no standalone entity XSD for them:

```text
DocumentPublicatie
DocumentPublicatieMetadata
FractieAanvullendGegeven
```

They are intentionally not modeled as importable SyncFeed entities until an
official source XSD exists. The check command prints these documented gaps so
the mismatch remains visible.
