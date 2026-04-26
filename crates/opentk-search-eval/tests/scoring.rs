use opentk_search_eval::{
    score_query_result, validate_quality_report, EndpointVerificationReport, QueryCategory,
    QueryResultScore, RawQueryResult, SearchDiskUsageReport, SearchDocument, SearchEvaluationInput,
    SearchLatencyReport, SearchQualityBenchmark, SearchQualityReport, SearchQualityThresholds,
    SearchQueryCase, SearchUpdateCase, UpdateResult,
};

#[test]
fn exact_top_hit_scores_higher_than_top_three_relevance() {
    let query = SearchQueryCase {
        id: "q".to_string(),
        category: QueryCategory::Exact,
        query: "35567-12".to_string(),
        expected_top: "expected".to_string(),
        acceptable_results: vec!["expected".to_string()],
        required_terms: Vec::new(),
    };

    let top_hit = score_query_result(
        &query,
        RawQueryResult {
            query_id: "q".to_string(),
            category: QueryCategory::Exact,
            result_ids: vec!["expected".to_string(), "other".to_string()],
            snippet_or_highlight: true,
            typo_tolerance_evidence: None,
        },
    )
    .expect("top hit should score");
    let top_three = score_query_result(
        &query,
        RawQueryResult {
            query_id: "q".to_string(),
            category: QueryCategory::Exact,
            result_ids: vec!["other".to_string(), "expected".to_string()],
            snippet_or_highlight: true,
            typo_tolerance_evidence: None,
        },
    )
    .expect("top-three hit should score");

    assert!(top_hit.quality_score > top_three.quality_score);
    assert!(top_hit.top_hit);
    assert!(!top_three.top_hit);
}

#[test]
fn typo_queries_record_typo_tolerance_evidence() {
    let query = SearchQueryCase {
        id: "q-typo".to_string(),
        category: QueryCategory::FuzzyTypo,
        query: "Fatma Kaija".to_string(),
        expected_top: "person-fatima-kaya".to_string(),
        acceptable_results: vec!["person-fatima-kaya".to_string()],
        required_terms: Vec::new(),
    };

    let score = score_query_result(
        &query,
        RawQueryResult {
            query_id: "q-typo".to_string(),
            category: QueryCategory::FuzzyTypo,
            result_ids: vec!["person-fatima-kaya".to_string()],
            snippet_or_highlight: true,
            typo_tolerance_evidence: Some("fatma matched fatima".to_string()),
        },
    )
    .expect("typo result should score");

    assert!(score.typo_tolerance_evidence.is_some());
}

#[test]
fn missing_expected_result_fails_evaluation() {
    let query = SearchQueryCase {
        id: "q-missing".to_string(),
        category: QueryCategory::DocumentText,
        query: "woningbouw".to_string(),
        expected_top: "doc-expected".to_string(),
        acceptable_results: vec!["doc-expected".to_string()],
        required_terms: Vec::new(),
    };

    let error = score_query_result(
        &query,
        RawQueryResult {
            query_id: "q-missing".to_string(),
            category: QueryCategory::DocumentText,
            result_ids: vec!["doc-other".to_string()],
            snippet_or_highlight: false,
            typo_tolerance_evidence: None,
        },
    )
    .expect_err("missing expected result should fail");

    assert!(error.to_string().contains("did not return expected result"));
}

#[test]
fn quality_report_fails_when_expected_top_is_poorly_ranked() {
    let benchmark = SearchQualityBenchmark {
        input: SearchEvaluationInput {
            corpus: vec![SearchDocument {
                id: "doc-expected".to_string(),
                title: "Expected".to_string(),
                summary: None,
                source_category: "Document".to_string(),
                source_id: "11111111-1111-4111-8111-111111111111".to_string(),
                document_number: Some("35567-12".to_string()),
                date: None,
                url: None,
                extracted_text: "expected".to_string(),
                extracted_html: "<p>expected</p>".to_string(),
                entity_metadata: Vec::new(),
                relation_labels: Vec::new(),
            }],
            queries: vec![SearchQueryCase {
                id: "q-quality".to_string(),
                category: QueryCategory::Exact,
                query: "35567-12".to_string(),
                expected_top: "doc-expected".to_string(),
                acceptable_results: vec!["doc-expected".to_string()],
                required_terms: vec!["35567-12".to_string()],
            }],
            update_case: SearchUpdateCase {
                changed_document: SearchDocument {
                    id: "doc-updated".to_string(),
                    title: "Updated".to_string(),
                    summary: None,
                    source_category: "Document".to_string(),
                    source_id: "11111111-1111-4111-8111-111111111111".to_string(),
                    document_number: None,
                    date: None,
                    url: None,
                    extracted_text: "updated".to_string(),
                    extracted_html: String::new(),
                    entity_metadata: Vec::new(),
                    relation_labels: Vec::new(),
                },
                deleted_document_id: "doc-deleted".to_string(),
                changed_query: "updated".to_string(),
                deleted_query: "deleted".to_string(),
            },
        },
        thresholds: SearchQualityThresholds {
            min_query_score: 0.90,
            min_total_score: 0.90,
            max_p95_latency_ms: 100,
            max_disk_bytes_per_record: 10_000,
        },
    };
    let report = SearchQualityReport {
        record_count: 1,
        query_results: vec![QueryResultScore {
            query_id: "q-quality".to_string(),
            category: QueryCategory::Exact,
            result_ids: vec!["doc-expected".to_string()],
            expected_top_rank: Some(1),
            top_hit: false,
            top_3_recall: true,
            typo_tolerance_evidence: None,
            snippet_or_highlight: true,
            quality_score: 0.80,
        }],
        latency: SearchLatencyReport::default(),
        disk: SearchDiskUsageReport::default(),
        update_result: UpdateResult::default(),
        endpoint_result: EndpointVerificationReport::default(),
    };

    let error = validate_quality_report(&benchmark, &report)
        .expect_err("poorly ranked expected top result should fail quality validation");

    assert!(error.to_string().contains("below minimum query score"));
}
