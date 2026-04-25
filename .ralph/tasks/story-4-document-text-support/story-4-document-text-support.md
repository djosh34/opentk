# Story 4: Document Text Support

Goal: fetch linked document assets, store upstream links in PostgreSQL, prefer official text/HTML/transcript versions from Tweede Kamer or other official government sources when available, extract text from binary formats only as a fallback, store document text/HTML in PostgreSQL, and expose the content through HTTP. Binary source files remain linked assets; official or extracted text/HTML becomes queryable API data.

Tasks:

- task-1-design-document-content-schema.md
- task-2-fetch-and-classify-document-assets.md
- task-3-extract-document-text-and-html.md
- task-4-expose-document-content-api.md
- task-5-deep-verify-document-content.md
