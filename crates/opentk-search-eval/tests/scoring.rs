use opentk_search_eval::{score_query_result, QueryCategory, RawQueryResult, SearchQueryCase};

#[test]
fn exact_top_hit_scores_higher_than_top_three_relevance() {
    let query = SearchQueryCase {
        id: "q".to_string(),
        category: QueryCategory::Exact,
        query: "35567-12".to_string(),
        expected_top: "expected".to_string(),
        acceptable_results: vec!["expected".to_string()],
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
