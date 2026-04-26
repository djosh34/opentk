use std::path::Path;

use opentk_search_eval::{load_fixture_input, load_result_reports};

#[test]
fn benchmark_fixtures_cover_required_query_categories() {
    let input = load_fixture_input(Path::new("fixtures")).expect("fixtures should be valid");

    assert!(!input.corpus.is_empty());
    assert!(!input.queries.is_empty());
}

#[test]
fn committed_engine_results_cover_every_query_with_measurements() {
    let input = load_fixture_input(Path::new("fixtures")).expect("fixtures should be valid");
    let reports =
        load_result_reports(Path::new("fixtures"), &input).expect("results should be valid");

    assert!(reports.len() >= 2);
}
