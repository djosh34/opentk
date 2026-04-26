use std::fs;

#[test]
fn investigation_doc_records_measured_recommendation() {
    let document = fs::read_to_string("../../docs/search-engine-investigation.md")
        .expect("search investigation doc should exist");

    for required in [
        "crates/opentk-search-eval/fixtures/results/meilisearch.json",
        "crates/opentk-search-eval/fixtures/results/tantivy.json",
        "Recommendation",
        "Meilisearch",
        "disk",
        "build",
        "update",
        "quality",
        "Story 5 Tasks 2-4",
    ] {
        assert!(
            document.contains(required),
            "investigation doc should contain {required}"
        );
    }
}
