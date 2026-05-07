## Bug: Document content 404 is treated as empty <status>not_started</status> <passes>false</passes> <priority>medium</priority>

<description>
The UI calls `/documents/{id}/content` for document detail pages and treats HTTP 404 as `null`.
This was detected while verifying `http://127.0.0.1:4321/detail?api=%2Fdocuments%2Fda29f021-98a7-4466-8889-4e9e62ae7c12&q=Fleur+Agema+onverzekerden+Zembla`: the document detail loaded, but DevTools still showed `GET https://api.watdoenzedaar.nl/documents/da29f021-98a7-4466-8889-4e9e62ae7c12/content [404]`.
The current code silently maps that 404 to missing content, which hides whether the backend is missing extracted content, the UI is asking for content for a document that should not have it, or the endpoint contract is wrong.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
