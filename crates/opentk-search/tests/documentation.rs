use std::fs;
use std::path::Path;

#[test]
fn search_indexing_pipeline_document_covers_operational_design() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = manifest_dir.parent().unwrap().parent().unwrap();
    let documentation = fs::read_to_string(workspace_dir.join("docs/search-indexing-pipeline.md"))
        .expect("search indexing design document must exist");

    for required in [
        "Backfill Indexing",
        "Incremental Indexing",
        "Failure Recovery",
        "Update And Delete Propagation",
        "API Result Shape",
        "Storage Estimate",
        "3,579 bytes",
        "716 bytes",
        "search_index_cursor",
        "search_index_failure",
    ] {
        assert!(
            documentation.contains(required),
            "design document must cover {required}"
        );
    }
}
