## Task: Story 4 Task 3 - Extract Document Text and HTML <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Store official text/HTML/transcript content from Tweede Kamer or other official government sources when available, and extract text and/or HTML from binary assets only as a fallback. The implementation must evaluate and choose reliable extraction tools/libraries for PDF, DOCX, and HTML inputs, then verify output against strict fixtures. The accepted quality bar is exact fixture matching for supported formats; any mismatch is a failing test and must be fixed or the format must be marked unsupported with an explicit error status.

This task must treat official text/HTML/transcript sources as preferred because official debate/document text is prepared by professional sources and is expected to be more reliable than binary parsing. Binary extraction must never be considered "good enough" when it produces small word mistakes, missing text, ordering errors, encoding problems, or layout artifacts. Those are defects. It must record provenance, official-source indicator, tool/library version where extraction is used, source hash, output hash, and validation status. It must support repeated extraction deterministically.

In scope: official text/HTML/transcript source preference, PDF extraction fallback, DOCX extraction fallback, HTML storage/normalization, deterministic text normalization rules, provenance, validation fixtures, and error status for unsupported or mismatching inputs. Out of scope: search indexing.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add golden fixtures for PDF, DOCX, and HTML where expected output mismatches fail, then make them pass.
- [x] Official text/HTML/transcript source is selected over PDF/DOCX extraction when both are available.
- [x] Stored provenance records whether content came from an official text/HTML source or binary extraction fallback.
- [x] Extraction stores exact expected text for supported PDF fixtures.
- [x] Extraction stores exact expected text for supported DOCX fixtures.
- [x] HTML assets store retrievable HTML and extracted plain text where applicable.
- [x] Extraction output is deterministic across repeated runs.
- [x] Word mistakes, missing text, ordering errors, encoding errors, and layout artifacts fail tests and record explicit extraction error status.
- [x] Extraction provenance includes source hash, output hash, tool/library name, and tool/library version.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — not applicable; task is not story-ending and did not change long-test selection
</acceptance_criteria>

<plan>
.ralph/tasks/story-4-document-text-support/task-3-extract-document-text-and-html_plans/document-text-and-html-extraction-plan.md
</plan>

NOW EXECUTE
