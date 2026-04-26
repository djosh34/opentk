use std::path::Path;

use opentk_search::SearchIndexOperation;
use opentk_search_eval::{
    load_fixture_input, load_quality_benchmark, load_result_reports, run_quality_benchmark,
    validate_quality_report,
};

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

#[test]
fn deep_quality_benchmark_covers_required_fixture_categories_and_content() {
    let benchmark =
        load_quality_benchmark(Path::new("fixtures")).expect("quality benchmark should be valid");

    assert!(benchmark.input.corpus.len() >= 5);
    assert!(
        benchmark
            .input
            .corpus
            .iter()
            .filter(|document| document.document_number.is_some())
            .count()
            >= 3
    );
    for category in [
        opentk_search_eval::QueryCategory::FuzzyTypo,
        opentk_search_eval::QueryCategory::Exact,
        opentk_search_eval::QueryCategory::DocumentText,
        opentk_search_eval::QueryCategory::HtmlContent,
        opentk_search_eval::QueryCategory::EntityMetadata,
        opentk_search_eval::QueryCategory::Mixed,
    ] {
        assert!(
            benchmark
                .input
                .queries
                .iter()
                .any(|query| query.category == category),
            "missing {category:?}"
        );
    }
    assert!(benchmark
        .input
        .queries
        .iter()
        .all(|query| !query.required_terms.is_empty()));
}

#[test]
fn deep_quality_runner_measures_latency_disk_and_update_propagation() {
    let benchmark =
        load_quality_benchmark(Path::new("fixtures")).expect("quality benchmark should load");
    let index_dir = Path::new("../../target/search-eval-tests/deep-quality-runner");
    let _ = std::fs::remove_dir_all(index_dir);

    let report =
        run_quality_benchmark(&benchmark, index_dir).expect("quality benchmark should pass");

    assert_eq!(report.record_count, benchmark.input.corpus.len());
    assert_eq!(
        report.latency.query_latencies_ms.len(),
        benchmark.input.queries.len()
    );
    assert!(report.latency.p95_latency_ms > 0);
    assert!(report.disk.index_bytes > 0);
    assert!(report.disk.bytes_per_record > 0);
    assert_eq!(
        report.update_result.changed_query_top_result.as_deref(),
        Some(benchmark.input.update_case.changed_document.id.as_str())
    );
    assert!(!report
        .update_result
        .deleted_query_result_ids
        .contains(&benchmark.input.update_case.deleted_document_id));
}

#[test]
fn quality_validation_fails_when_latency_or_disk_thresholds_regress() {
    let benchmark =
        load_quality_benchmark(Path::new("fixtures")).expect("quality benchmark should load");
    let index_dir = Path::new("../../target/search-eval-tests/deep-quality-thresholds");
    let _ = std::fs::remove_dir_all(index_dir);
    let mut report =
        run_quality_benchmark(&benchmark, index_dir).expect("quality benchmark should pass");

    report.latency.p95_latency_ms = benchmark.thresholds.max_p95_latency_ms + 1;
    let latency_error = validate_quality_report(&benchmark, &report)
        .expect_err("latency above threshold should fail");
    assert!(latency_error.to_string().contains("p95 latency"));

    report.latency.p95_latency_ms = 1;
    report.disk.bytes_per_record = benchmark.thresholds.max_disk_bytes_per_record + 1;
    let disk_error = validate_quality_report(&benchmark, &report)
        .expect_err("disk usage above threshold should fail");
    assert!(disk_error.to_string().contains("bytes per record"));
}

#[test]
fn benchmark_documents_map_to_production_search_operations() {
    let benchmark =
        load_quality_benchmark(Path::new("fixtures")).expect("quality benchmark should load");
    let document = benchmark
        .input
        .corpus
        .iter()
        .find(|document| document.id == "doc-2024-35567")
        .expect("expected document fixture");

    let operation = document
        .to_index_operation()
        .expect("benchmark document should map");

    let SearchIndexOperation::Upsert(index_document) = operation else {
        panic!("benchmark fixture should map to an upsert");
    };
    assert_eq!(
        index_document.key,
        format!("Document:{}", document.source_id)
    );
    assert_eq!(
        index_document.entity_kind,
        opentk_search::SearchEntityKind::Document
    );
    assert_eq!(index_document.document_number.as_deref(), Some("35567-12"));
    assert!(index_document
        .metadata_text
        .iter()
        .any(|value| value.contains("GroenLinks-PvdA")));
    assert!(index_document
        .relation_labels
        .iter()
        .any(|value| value.contains("commissie Digitale Zaken")));
}
