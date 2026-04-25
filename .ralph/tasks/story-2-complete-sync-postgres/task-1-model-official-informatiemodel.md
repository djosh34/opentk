## Task: Story 2 Task 1 - Model Official Informatiemodel and XSDs <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the local source-of-truth model for every Tweede Kamer SyncFeed entity type from the official informatiemodel and XSDs. The official informatiemodel page lists entity types and relationships, and states that XSDs define entity fields, order, datatypes, and relations. This task must turn those official sources into repository-owned schema metadata that later migrations, parsers, and tests use.

The resulting model must cover all official entity types, including Activiteit, ActiviteitActor, Agendapunt, Besluit, Commissie, CommissieContactinformatie, CommissieZetel, CommissieZetelVastPersoon, CommissieZetelVastVacature, CommissieZetelVervangerPersoon, CommissieZetelVervangerVacature, Document, DocumentActor, DocumentPublicatie, DocumentPublicatieMetadata, DocumentVersie, Fractie, FractieZetel, FractieZetelPersoon, FractieZetelVacature, Kamerstukdossier, Persoon, PersoonContactinformatie, PersoonGeschenk, PersoonLoopbaan, PersoonNevenfunctie, PersoonNevenfunctieInkomsten, PersoonOnderwijs, PersoonReis, Reservering, Stemming, Toezegging, Vergadering, Verslag, Zaak, ZaakActor, and Zaal, plus any additional current official types found in the XSDs.

In scope: fetch or vendor the official XSDs in a reproducible way, parse/extract entity definitions, attributes, scalar fields, relationship fields, multiplicity, datatypes, and category names; create tests that compare the local model to the official sources; document how to refresh the model when the official docs change. Out of scope: writing the sync runner itself.

Official source documentation:

- Informatiemodel: https://opendata.tweedekamer.nl/documentatie/informatiemodel
- SyncFeed API: https://opendata.tweedekamer.nl/documentatie/syncfeed-api
- XSD link from informatiemodel page: https://github.com/TweedeKamerDerStaten-Generaal/OpenDataPortaal



</description>


<acceptance_criteria>
- [x] Red/green TDD: add a fixture test that fails when a known entity/field/relation from the official model is missing, then make it pass.
- [x] Local schema metadata covers every current official entity type and relationship from the informatiemodel/XSDs.
- [x] Datatypes and multiplicities are captured sufficiently to generate PostgreSQL migrations and parser assertions.
- [x] A model refresh command or documented procedure exists and fails clearly on source mismatch.
- [x] Tests assert that every SyncFeed category planned for import has a model entry.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>
.ralph/tasks/story-2-complete-sync-postgres/task-1-model-official-informatiemodel_plans/official-schema-model-plan.md
</plan>

NOW EXECUTE
