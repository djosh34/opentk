## Plan: Official SyncFeed Schema Model

<status>planned</status>

### Sources Checked

- Informatiemodel page: `https://opendata.tweedekamer.nl/documentatie/informatiemodel`
- SyncFeed API page: `https://opendata.tweedekamer.nl/documentatie/syncfeed-api`
- Official XSD repository: `https://github.com/TweedeKamerDerStaten-Generaal/OpenDataPortaal`
- Official repository head observed during planning: `f2439f4a5b8bafa116245420e295d01c879d6b5f`

The SyncFeed documentation says the SyncFeed datamodel is the same as the informatiemodel. The informatiemodel says the XSDs define possible fields, order, datatypes, relations, and related schema details. Therefore the repository-owned model should be generated from vendored official XSD files and verified against the public informatiemodel category list.

### Boundary Design

- Keep source-neutral schema metadata in `opentk-core`; it is domain metadata needed by migrations, parser assertions, and future API surfaces.
- Keep official XSD vendoring and refresh tooling in `opentk-sync`; fetching/parsing upstream SyncFeed source files belongs at the sync boundary.
- Expose a small public model interface from `opentk-core`, likely:
  - `official_schema::entity_types() -> &'static [EntityType]`
  - `official_schema::entity_named(name: &str) -> Option<&'static EntityType>`
  - `EntityType { category, xml_element, rust_name, base, fields }`
  - `Field { name, kind, min_occurs, max_occurs, nillable, xsd_type, order }`
  - `FieldKind::Attribute | FieldKind::Relation { target: RelationTarget }`
  - `Occurs::One | Occurs::Unbounded | Occurs::Exactly(u32)`
- Model XSD base attributes (`id`, `verwijderd`, `bijgewerkt`, plus download attributes for `downloadEntiteitType`) explicitly instead of scattering special cases through migration/parser code later.
- Store generated metadata as checked-in Rust data or checked-in JSON plus a thin typed loader. Prefer checked-in Rust if the generated data is compact enough, because downstream crates get typed constants without runtime parsing.
- Do not build migration/table naming into this task. Capture enough stable metadata for later migrations to decide SQL names without coupling the official model to one database shape.

### Source Vendoring And Refresh Interface

- Vendor the official XSD files under a repository-owned path such as `crates/opentk-sync/official_xsd/tkdata-v1-0/` or `official_sources/tweedekamer/xsd/`.
- Add a refresh command that:
  - downloads the GitHub tree from `TweedeKamerDerStaten-Generaal/OpenDataPortaal`;
  - pins the observed commit SHA in a small manifest;
  - writes all `xsd/tkData-v1-0*.xsd` files reproducibly;
  - regenerates the local schema metadata;
  - fails clearly if public informatiemodel categories and XSD entity files disagree.
- The command can initially be a Rust test-support binary or `xtask`-style small CLI. Pick the simplest shape that integrates cleanly with `make check` and `make test` without requiring network access by default.
- Network is only for manual refresh. Default tests compare checked-in metadata to checked-in official sources and fail on local drift.

### TDD Execution Plan

Use vertical red-green slices, one behavior at a time:

- [x] RED 1: Add one public-interface test in `opentk-core` proving a known official entity is present with a known scalar field and relation. Use `Document` because it exercises `downloadEntiteitType`, scalar fields, one-to-one relation (`kamerstukdossier`), and unbounded relations (`activiteit`, `agendapunt`, `zaak`, `bronDocument`). Confirm it fails because no official schema model exists.
- [x] GREEN 1: Add the minimal typed schema API and enough `Document` metadata to pass.
- [x] RED 2: Add a test that parses the vendored `document.xsd` and asserts the local `Document` model matches field order, `minOccurs`, `maxOccurs`, `nillable`, and raw XSD types from the source. Confirm it fails before wiring the parser/generator.
- [x] GREEN 2: Implement the XSD extraction path using structured XML parsing, not string matching. Classify `referentieLiteral` / `referentieBijgewerktLiteral` fields as relations and scalar XSD/simple wrapper types as attributes.
- [x] RED 3: Add a test over all vendored entity XSD files proving every top-level entity element has a local model entry and every local model entry has a source XSD. Confirm it fails until all entities are generated/filled.
- [x] GREEN 3: Generate or check in metadata for every current entity XSD:
  `Activiteit`, `ActiviteitActor`, `Agendapunt`, `Besluit`, `Commissie`, `CommissieContactinformatie`, `CommissieZetel`, `CommissieZetelVastPersoon`, `CommissieZetelVastVacature`, `CommissieZetelVervangerPersoon`, `CommissieZetelVervangerVacature`, `Document`, `DocumentActor`, `DocumentVersie`, `Fractie`, `FractieZetel`, `FractieZetelPersoon`, `FractieZetelVacature`, `Kamerstukdossier`, `Persoon`, `PersoonContactinformatie`, `PersoonGeschenk`, `PersoonLoopbaan`, `PersoonNevenfunctie`, `PersoonNevenfunctieInkomsten`, `PersoonOnderwijs`, `PersoonReis`, `Reservering`, `Stemming`, `Toezegging`, `Vergadering`, `Verslag`, `Zaak`, `ZaakActor`, and `Zaal`.
- [x] RED 4: Add coverage for task-mentioned categories that are documented on the public informatiemodel but are not standalone entity XSDs in the observed tree, specifically `DocumentPublicatie`, `DocumentPublicatieMetadata`, and any `FractieAanvullendGegeven` mismatch found during implementation. The expected behavior is a clear source-mismatch report unless the refresh discovers current official XSDs for them.
- [x] GREEN 4: Represent mismatch diagnostics in one place, either as explicit expected source gaps in the manifest or by adding the missing official files if refresh finds them. Do not swallow this mismatch.
- [x] RED 5: Add a refresh/check command test that intentionally mutates a copied fixture or manifest and proves the command fails clearly on source mismatch.
- [x] GREEN 5: Implement the refresh/check command and document the manual refresh procedure.
- [x] REFACTOR: Run the improve-code-boundaries review. Remove any duplicate DTO shape between parser output and domain model; keep one schema metadata type and one conversion boundary from XSD extraction into that type.

### Documentation

- Add a short `docs/official-schema-model.md` or equivalent section documenting:
  - vendored source location;
  - pinned upstream commit;
  - refresh command;
  - why default tests are offline;
  - how source mismatches are reported.

### Checks

- Run `make check`.
- Run `make lint`.
- Run `make test`.
- Do not run `make test-long` for this task unless implementation reveals it is explicitly story-ending or long-lane selection changes.

### Open Design Risks To Verify During Execution

- The public informatiemodel table observed during planning mentions `FractieAanvullendGegeven`, but the official GitHub XSD tree at `f2439f4a5b8bafa116245420e295d01c879d6b5f` did not expose `tkData-v1-0-fractieaanvullendgegeven.xsd`.
- The task acceptance list includes `DocumentPublicatie` and `DocumentPublicatieMetadata`, while the observed XSD tree exposes `documentversie.xsd` but no standalone `documentpublicatie.xsd` or `documentpublicatiemetadata.xsd`.
- If current official sources disagree in the same way during execution, switch the task back to `TO BE VERIFIED` only if the interface/types need to change. If the planned mismatch diagnostic covers it, continue and make the diagnostic explicit.

NOW EXECUTE
