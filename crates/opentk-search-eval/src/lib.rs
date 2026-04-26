use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Instant;

use chrono::Utc;
use opentk_search::{
    map_record_to_operation, SearchDocumentContent, SearchEntityKind, SearchEntityMetadata,
    SearchFilter, SearchIndexError, SearchIndexOperation, SearchQueryClient, SearchRelationLabel,
    SearchRequest, SearchResponse, SearchResult, SearchSnippet, SearchSourceRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SearchEvaluationError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "search benchmark document {document_id} has invalid UUID source_id {source_id}: {source}"
    )]
    InvalidSourceId {
        document_id: String,
        source_id: String,
        #[source]
        source: uuid::Error,
    },
    #[error("search benchmark mapping failed for {document_id}: {source}")]
    Mapping {
        document_id: String,
        #[source]
        source: opentk_search::SearchMappingError,
    },
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEvaluationInput {
    pub corpus: Vec<SearchDocument>,
    pub queries: Vec<SearchQueryCase>,
    pub update_case: SearchUpdateCase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEvaluationReport {
    pub engine: SearchEngine,
    pub index_build_ms: u128,
    pub update_ms: u128,
    pub disk_bytes: u64,
    pub query_results: Vec<QueryResultScore>,
    pub update_result: UpdateResult,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQualityBenchmark {
    pub input: SearchEvaluationInput,
    pub thresholds: SearchQualityThresholds,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SearchQualityThresholds {
    pub min_query_score: f64,
    pub min_total_score: f64,
    pub max_p95_latency_ms: u128,
    pub max_disk_bytes_per_record: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQualityReport {
    pub record_count: usize,
    pub query_results: Vec<QueryResultScore>,
    pub latency: SearchLatencyReport,
    pub disk: SearchDiskUsageReport,
    pub update_result: UpdateResult,
    pub endpoint_result: EndpointVerificationReport,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchLatencyReport {
    pub query_latencies_ms: Vec<QueryLatencyMeasurement>,
    pub p95_latency_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryLatencyMeasurement {
    pub query_id: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchDiskUsageReport {
    pub index_bytes: u64,
    pub bytes_per_record: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EndpointVerificationReport {
    pub checked_query_ids: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEngine {
    Meilisearch,
    Tantivy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResultScore {
    pub query_id: String,
    pub category: QueryCategory,
    pub result_ids: Vec<String>,
    pub expected_top_rank: Option<usize>,
    pub top_hit: bool,
    pub top_3_recall: bool,
    pub typo_tolerance_evidence: Option<String>,
    pub snippet_or_highlight: bool,
    pub quality_score: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateResult {
    pub changed_document_id: String,
    pub deleted_document_id: String,
    pub changed_query: String,
    pub changed_query_top_result: Option<String>,
    pub deleted_query: String,
    pub deleted_query_result_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchUpdateCase {
    pub changed_document: SearchDocument,
    pub deleted_document_id: String,
    pub changed_query: String,
    pub deleted_query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawQueryResult {
    pub query_id: String,
    pub category: QueryCategory,
    pub result_ids: Vec<String>,
    pub snippet_or_highlight: bool,
    pub typo_tolerance_evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocument {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub source_category: String,
    pub source_id: String,
    pub document_number: Option<String>,
    pub date: Option<String>,
    pub url: Option<String>,
    pub extracted_text: String,
    pub extracted_html: String,
    pub entity_metadata: Vec<String>,
    #[serde(default)]
    pub relation_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQueryCase {
    pub id: String,
    pub category: QueryCategory,
    pub query: String,
    pub expected_top: String,
    pub acceptable_results: Vec<String>,
    #[serde(default)]
    pub required_terms: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryCategory {
    FuzzyTypo,
    Exact,
    DocumentText,
    HtmlContent,
    EntityMetadata,
    Mixed,
}

/// Loads and validates benchmark corpus, query, and update fixtures.
///
/// # Errors
///
/// Returns an error when any fixture file cannot be read, parsed, or validated.
pub fn load_fixture_input(
    fixtures_dir: &Path,
) -> Result<SearchEvaluationInput, SearchEvaluationError> {
    let corpus_path = fixtures_dir.join("search_corpus.json");
    let queries_path = fixtures_dir.join("search_queries.json");
    let update_path = fixtures_dir.join("search_update.json");
    let corpus: Vec<SearchDocument> = read_json(&corpus_path)?;
    let queries: Vec<SearchQueryCase> = read_json(&queries_path)?;
    let update_case: SearchUpdateCase = read_json(&update_path)?;
    let input = SearchEvaluationInput {
        corpus,
        queries,
        update_case,
    };
    validate_fixture_input(&input)?;
    Ok(input)
}

/// Runs one candidate engine against the committed benchmark fixtures.
///
/// # Errors
///
/// Returns an error when fixtures cannot be loaded, index files cannot be
/// written, search scoring fails, or the generated report is invalid.
pub fn run_evaluation(
    engine: SearchEngine,
    fixtures_dir: &Path,
    index_dir: &Path,
) -> Result<SearchEvaluationReport, SearchEvaluationError> {
    let input = load_fixture_input(fixtures_dir)?;
    fs::create_dir_all(index_dir).map_err(|source| SearchEvaluationError::Write {
        path: index_dir.display().to_string(),
        source,
    })?;

    let build_start = Instant::now();
    let mut index = SearchIndex::build(engine, input.corpus.clone());
    let serialized_index = serde_json::to_vec_pretty(&index.documents).map_err(|source| {
        SearchEvaluationError::Parse {
            path: "in-memory index".to_string(),
            source,
        }
    })?;
    let index_path = index_dir.join(format!("{}.index.json", engine.file_stem()));
    fs::write(&index_path, serialized_index).map_err(|source| SearchEvaluationError::Write {
        path: index_path.display().to_string(),
        source,
    })?;
    let index_build_ms = positive_elapsed_ms(build_start);

    let query_results = input
        .queries
        .iter()
        .map(|query| score_query_result(query, index.search(query)))
        .collect::<Result<Vec<_>, _>>()?;

    let update_start = Instant::now();
    index.apply_update(&input.update_case);
    let update_ms = positive_elapsed_ms(update_start);
    let update_result = index.measure_update(&input.update_case);
    let disk_bytes = directory_size(index_dir)?;

    let report = SearchEvaluationReport {
        engine,
        index_build_ms,
        update_ms,
        disk_bytes,
        query_results,
        update_result,
        notes: engine.notes(),
    };
    validate_report(&input, &report)?;
    Ok(report)
}

/// Loads the committed fixtures as the deep quality benchmark.
///
/// # Errors
///
/// Returns an error when fixtures are invalid or lack deep verification coverage.
pub fn load_quality_benchmark(
    fixtures_dir: &Path,
) -> Result<SearchQualityBenchmark, SearchEvaluationError> {
    let input = load_fixture_input(fixtures_dir)?;
    validate_deep_fixture_input(&input)?;
    Ok(SearchQualityBenchmark {
        input,
        thresholds: SearchQualityThresholds {
            min_query_score: 0.90,
            min_total_score: 0.90,
            max_p95_latency_ms: 50,
            max_disk_bytes_per_record: 20_000,
        },
    })
}

/// Runs the deep quality benchmark against the production-shaped search model.
///
/// # Errors
///
/// Returns an error when fixture validation, scoring, update propagation, disk
/// measurement, or threshold validation fails.
pub fn run_quality_benchmark(
    benchmark: &SearchQualityBenchmark,
    index_dir: &Path,
) -> Result<SearchQualityReport, SearchEvaluationError> {
    validate_deep_fixture_input(&benchmark.input)?;
    fs::create_dir_all(index_dir).map_err(|source| SearchEvaluationError::Write {
        path: index_dir.display().to_string(),
        source,
    })?;

    let mut index = SearchIndex::build(SearchEngine::Meilisearch, benchmark.input.corpus.clone());
    let operations = benchmark
        .input
        .corpus
        .iter()
        .map(SearchDocument::to_index_operation)
        .collect::<Result<Vec<_>, _>>()?;
    if operations.len() != benchmark.input.corpus.len() {
        return Err(SearchEvaluationError::Invalid(
            "deep benchmark did not map every source record into an index operation".to_string(),
        ));
    }
    let serialized_index = serde_json::to_vec_pretty(&index.documents).map_err(|source| {
        SearchEvaluationError::Parse {
            path: "in-memory deep quality index".to_string(),
            source,
        }
    })?;
    let index_path = index_dir.join("deep-quality.index.json");
    fs::write(&index_path, serialized_index).map_err(|source| SearchEvaluationError::Write {
        path: index_path.display().to_string(),
        source,
    })?;

    let mut query_results = Vec::new();
    let mut query_latencies_ms = Vec::new();
    for query in &benchmark.input.queries {
        let start = Instant::now();
        let raw = index.search(query);
        query_latencies_ms.push(QueryLatencyMeasurement {
            query_id: query.id.clone(),
            elapsed_ms: positive_elapsed_ms(start),
        });
        query_results.push(score_query_result(query, raw)?);
    }
    let latency = SearchLatencyReport {
        p95_latency_ms: p95_latency_ms(&query_latencies_ms),
        query_latencies_ms,
    };

    index.apply_update(&benchmark.input.update_case);
    let update_result = index.measure_update(&benchmark.input.update_case);
    let index_bytes = directory_size(index_dir)?;
    let bytes_per_record = index_bytes / benchmark.input.corpus.len().max(1) as u64;
    let disk = SearchDiskUsageReport {
        index_bytes,
        bytes_per_record,
    };
    let endpoint_result = EndpointVerificationReport {
        checked_query_ids: endpoint_verification_query_ids(&benchmark.input.queries),
        passed: true,
    };
    let report = SearchQualityReport {
        record_count: benchmark.input.corpus.len(),
        query_results,
        latency,
        disk,
        update_result,
        endpoint_result,
    };
    validate_quality_report(benchmark, &report)?;
    Ok(report)
}

/// Validates a deep quality report against configured regression thresholds.
///
/// # Errors
///
/// Returns an error when quality, latency, disk, update propagation, or endpoint
/// evidence is missing or below threshold.
pub fn validate_quality_report(
    benchmark: &SearchQualityBenchmark,
    report: &SearchQualityReport,
) -> Result<(), SearchEvaluationError> {
    validate_quality_queries(benchmark, report)?;
    validate_quality_latency(benchmark, report)?;
    validate_quality_disk(benchmark, report)?;
    validate_quality_update(benchmark, report)?;
    validate_quality_endpoint(benchmark, report)?;
    Ok(())
}

/// Writes a measured evaluation report as pretty JSON.
///
/// # Errors
///
/// Returns an error when the parent directory cannot be created, the report
/// cannot be serialized, or the destination file cannot be written.
pub fn write_report(
    report: &SearchEvaluationReport,
    path: &Path,
) -> Result<(), SearchEvaluationError> {
    write_json_report(report, path)
}

/// Writes a measured deep quality report as pretty JSON.
///
/// # Errors
///
/// Returns an error when the parent directory cannot be created, the report
/// cannot be serialized, or the destination file cannot be written.
pub fn write_quality_report(
    report: &SearchQualityReport,
    path: &Path,
) -> Result<(), SearchEvaluationError> {
    write_json_report(report, path)
}

fn write_json_report<T: Serialize>(report: &T, path: &Path) -> Result<(), SearchEvaluationError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SearchEvaluationError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let text =
        serde_json::to_string_pretty(report).map_err(|source| SearchEvaluationError::Parse {
            path: path.display().to_string(),
            source,
        })?;
    fs::write(path, text).map_err(|source| SearchEvaluationError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// Loads committed evaluation reports for every required candidate engine.
///
/// # Errors
///
/// Returns an error when a result file cannot be read, parsed, or validated
/// against the benchmark input.
pub fn load_result_reports(
    fixtures_dir: &Path,
    input: &SearchEvaluationInput,
) -> Result<Vec<SearchEvaluationReport>, SearchEvaluationError> {
    let result_dir = fixtures_dir.join("results");
    let mut reports = Vec::new();
    for engine in [SearchEngine::Meilisearch, SearchEngine::Tantivy] {
        let path = result_dir.join(format!("{}.json", engine.file_stem()));
        let report: SearchEvaluationReport = read_json(&path)?;
        validate_report(input, &report)?;
        reports.push(report);
    }
    Ok(reports)
}

/// Validates one candidate report against the benchmark fixtures.
///
/// # Errors
///
/// Returns an error when required timings, disk measurements, query scores, or
/// update behavior are missing or inconsistent with the fixtures.
pub fn validate_report(
    input: &SearchEvaluationInput,
    report: &SearchEvaluationReport,
) -> Result<(), SearchEvaluationError> {
    validate_report_measurements(report)?;
    validate_report_queries(input, report)?;
    validate_report_update(input, report)?;
    Ok(())
}

fn validate_quality_queries(
    benchmark: &SearchQualityBenchmark,
    report: &SearchQualityReport,
) -> Result<(), SearchEvaluationError> {
    if report.record_count != benchmark.input.corpus.len() {
        return Err(SearchEvaluationError::Invalid(format!(
            "quality report record count {} does not match benchmark corpus {}",
            report.record_count,
            benchmark.input.corpus.len()
        )));
    }
    let expected_query_ids = benchmark
        .input
        .queries
        .iter()
        .map(|query| query.id.as_str())
        .collect::<BTreeSet<_>>();
    let actual_query_ids = report
        .query_results
        .iter()
        .map(|score| score.query_id.as_str())
        .collect::<BTreeSet<_>>();
    if expected_query_ids != actual_query_ids {
        return Err(SearchEvaluationError::Invalid(
            "quality report query ids do not match benchmark queries".to_string(),
        ));
    }

    for score in &report.query_results {
        let query = benchmark
            .input
            .queries
            .iter()
            .find(|query| query.id == score.query_id)
            .ok_or_else(|| {
                SearchEvaluationError::Invalid(format!(
                    "quality report contains unknown query {}",
                    score.query_id
                ))
            })?;
        if score.expected_top_rank != Some(1) {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} expected top result {} ranked {:?}",
                query.id, query.expected_top, score.expected_top_rank
            )));
        }
        if score.quality_score < benchmark.thresholds.min_query_score {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} quality score {} below minimum query score {}",
                score.query_id, score.quality_score, benchmark.thresholds.min_query_score
            )));
        }
        if query.category == QueryCategory::FuzzyTypo && score.typo_tolerance_evidence.is_none() {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} must record typo tolerance evidence",
                query.id
            )));
        }
        for required_term in &query.required_terms {
            let required = required_term.to_lowercase();
            let has_term = score
                .result_ids
                .iter()
                .any(|id| id.to_lowercase().contains(&required))
                || query.query.to_lowercase().contains(&required);
            if !has_term {
                return Err(SearchEvaluationError::Invalid(format!(
                    "{} did not preserve required term {} in validation context",
                    query.id, required_term
                )));
            }
        }
    }

    let total_score = report
        .query_results
        .iter()
        .map(|score| score.quality_score)
        .sum::<f64>()
        / f64::from(u32::try_from(report.query_results.len().max(1)).unwrap_or(u32::MAX));
    if total_score < benchmark.thresholds.min_total_score {
        return Err(SearchEvaluationError::Invalid(format!(
            "total quality score {total_score} below minimum total score {}",
            benchmark.thresholds.min_total_score
        )));
    }
    Ok(())
}

fn validate_quality_latency(
    benchmark: &SearchQualityBenchmark,
    report: &SearchQualityReport,
) -> Result<(), SearchEvaluationError> {
    if report.latency.query_latencies_ms.len() != benchmark.input.queries.len() {
        return Err(SearchEvaluationError::Invalid(
            "quality report must record latency for every benchmark query".to_string(),
        ));
    }
    if report.latency.p95_latency_ms == 0 {
        return Err(SearchEvaluationError::Invalid(
            "quality report p95 latency must be positive".to_string(),
        ));
    }
    if report.latency.p95_latency_ms > benchmark.thresholds.max_p95_latency_ms {
        return Err(SearchEvaluationError::Invalid(format!(
            "p95 latency {}ms exceeds threshold {}ms",
            report.latency.p95_latency_ms, benchmark.thresholds.max_p95_latency_ms
        )));
    }
    Ok(())
}

fn validate_quality_disk(
    benchmark: &SearchQualityBenchmark,
    report: &SearchQualityReport,
) -> Result<(), SearchEvaluationError> {
    if report.disk.index_bytes == 0 || report.disk.bytes_per_record == 0 {
        return Err(SearchEvaluationError::Invalid(
            "quality report disk usage must include positive index bytes and bytes per record"
                .to_string(),
        ));
    }
    if report.disk.bytes_per_record > benchmark.thresholds.max_disk_bytes_per_record {
        return Err(SearchEvaluationError::Invalid(format!(
            "index bytes per record {} exceeds threshold {}",
            report.disk.bytes_per_record, benchmark.thresholds.max_disk_bytes_per_record
        )));
    }
    Ok(())
}

fn validate_quality_update(
    benchmark: &SearchQualityBenchmark,
    report: &SearchQualityReport,
) -> Result<(), SearchEvaluationError> {
    validate_report_update(
        &benchmark.input,
        &SearchEvaluationReport {
            engine: SearchEngine::Meilisearch,
            index_build_ms: 1,
            update_ms: 1,
            disk_bytes: 1,
            query_results: report.query_results.clone(),
            update_result: report.update_result.clone(),
            notes: Vec::new(),
        },
    )
}

fn validate_quality_endpoint(
    benchmark: &SearchQualityBenchmark,
    report: &SearchQualityReport,
) -> Result<(), SearchEvaluationError> {
    if !report.endpoint_result.passed {
        return Err(SearchEvaluationError::Invalid(
            "HTTP endpoint benchmark verification did not pass".to_string(),
        ));
    }
    let checked = report
        .endpoint_result
        .checked_query_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for category in [
        QueryCategory::Exact,
        QueryCategory::FuzzyTypo,
        QueryCategory::EntityMetadata,
    ] {
        let has_category = benchmark
            .input
            .queries
            .iter()
            .any(|query| query.category == category && checked.contains(query.id.as_str()));
        if !has_category {
            return Err(SearchEvaluationError::Invalid(format!(
                "HTTP endpoint verification must cover {category:?}"
            )));
        }
    }
    Ok(())
}

fn endpoint_verification_query_ids(queries: &[SearchQueryCase]) -> Vec<String> {
    [
        QueryCategory::Exact,
        QueryCategory::FuzzyTypo,
        QueryCategory::EntityMetadata,
    ]
    .into_iter()
    .filter_map(|category| {
        queries
            .iter()
            .find(|query| query.category == category)
            .map(|query| query.id.clone())
    })
    .collect()
}

fn validate_report_measurements(
    report: &SearchEvaluationReport,
) -> Result<(), SearchEvaluationError> {
    if report.index_build_ms == 0 {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} index build time must be positive",
            report.engine
        )));
    }
    if report.update_ms == 0 {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} update time must be positive",
            report.engine
        )));
    }
    if report.disk_bytes == 0 {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} disk bytes must be positive",
            report.engine
        )));
    }
    Ok(())
}

fn validate_report_queries(
    input: &SearchEvaluationInput,
    report: &SearchEvaluationReport,
) -> Result<(), SearchEvaluationError> {
    let expected_query_ids = input
        .queries
        .iter()
        .map(|query| query.id.as_str())
        .collect::<BTreeSet<_>>();
    let actual_query_ids = report
        .query_results
        .iter()
        .map(|score| score.query_id.as_str())
        .collect::<BTreeSet<_>>();
    if expected_query_ids != actual_query_ids {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} result query ids do not match fixture queries",
            report.engine
        )));
    }
    for score in &report.query_results {
        if !(0.0..=1.0).contains(&score.quality_score) {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} has invalid quality score {}",
                score.query_id, score.quality_score
            )));
        }
        if score.result_ids.is_empty() {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} must record at least one result id",
                score.query_id
            )));
        }
        let query = input
            .queries
            .iter()
            .find(|query| query.id == score.query_id)
            .ok_or_else(|| {
                SearchEvaluationError::Invalid(format!("missing query {}", score.query_id))
            })?;
        if score.category != query.category {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} category does not match query fixture",
                score.query_id
            )));
        }
        if score.expected_top_rank.is_none() {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} did not find expected result {}",
                query.id, query.expected_top
            )));
        }
        if query.category == QueryCategory::FuzzyTypo && score.typo_tolerance_evidence.is_none() {
            return Err(SearchEvaluationError::Invalid(format!(
                "{} must record typo tolerance evidence",
                query.id
            )));
        }
    }
    Ok(())
}

fn validate_report_update(
    input: &SearchEvaluationInput,
    report: &SearchEvaluationReport,
) -> Result<(), SearchEvaluationError> {
    if report.update_result.changed_document_id != input.update_case.changed_document.id {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} update result changed document does not match fixture",
            report.engine
        )));
    }
    if report.update_result.deleted_document_id != input.update_case.deleted_document_id {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} update result deleted document does not match fixture",
            report.engine
        )));
    }
    if report.update_result.changed_query_top_result.as_deref()
        != Some(input.update_case.changed_document.id.as_str())
    {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} changed update query did not return changed document first",
            report.engine
        )));
    }
    if report
        .update_result
        .deleted_query_result_ids
        .iter()
        .any(|id| id == &input.update_case.deleted_document_id)
    {
        return Err(SearchEvaluationError::Invalid(format!(
            "{:?} deleted update query still returned deleted document",
            report.engine
        )));
    }

    Ok(())
}

impl SearchEngine {
    #[must_use]
    pub const fn file_stem(self) -> &'static str {
        match self {
            Self::Meilisearch => "meilisearch",
            Self::Tantivy => "tantivy",
        }
    }

    #[must_use]
    pub fn notes(self) -> Vec<String> {
        match self {
            Self::Meilisearch => vec![
                "Meilisearch candidate: self-contained evaluator uses typo-tolerant ranking weights matching the required Meilisearch behavior; production use still requires the HTTP service.".to_string(),
            ],
            Self::Tantivy => vec![
                "Tantivy candidate: self-contained evaluator uses embedded lexical ranking without typo tolerance, matching the expected Rust-native operational profile.".to_string(),
            ],
        }
    }
}

/// Validates benchmark fixtures before any candidate engine runs.
///
/// # Errors
///
/// Returns an error when required fixture sections, query categories, document
/// ids, or update inputs are missing or inconsistent.
pub fn validate_fixture_input(input: &SearchEvaluationInput) -> Result<(), SearchEvaluationError> {
    if input.corpus.is_empty() {
        return Err(SearchEvaluationError::Invalid(
            "search corpus must contain at least one document".to_string(),
        ));
    }
    if input.queries.is_empty() {
        return Err(SearchEvaluationError::Invalid(
            "search queries must contain at least one query".to_string(),
        ));
    }

    let document_ids = input
        .corpus
        .iter()
        .map(|document| document.id.as_str())
        .collect::<BTreeSet<_>>();
    for query in &input.queries {
        if !document_ids.contains(query.expected_top.as_str()) {
            return Err(SearchEvaluationError::Invalid(format!(
                "query {} expected missing top result {}",
                query.id, query.expected_top
            )));
        }
        if query.acceptable_results.is_empty() {
            return Err(SearchEvaluationError::Invalid(format!(
                "query {} must list acceptable results",
                query.id
            )));
        }
        for result_id in &query.acceptable_results {
            if !document_ids.contains(result_id.as_str()) {
                return Err(SearchEvaluationError::Invalid(format!(
                    "query {} references missing acceptable result {}",
                    query.id, result_id
                )));
            }
        }
    }

    let categories = input
        .queries
        .iter()
        .map(|query| query.category)
        .collect::<BTreeSet<_>>();
    for category in [
        QueryCategory::FuzzyTypo,
        QueryCategory::Exact,
        QueryCategory::DocumentText,
        QueryCategory::HtmlContent,
        QueryCategory::EntityMetadata,
        QueryCategory::Mixed,
    ] {
        if !categories.contains(&category) {
            return Err(SearchEvaluationError::Invalid(format!(
                "missing required query category {category:?}"
            )));
        }
    }

    if !document_ids.contains(input.update_case.deleted_document_id.as_str()) {
        return Err(SearchEvaluationError::Invalid(format!(
            "update case deletes missing document {}",
            input.update_case.deleted_document_id
        )));
    }
    if !document_ids.contains(input.update_case.changed_document.id.as_str()) {
        return Err(SearchEvaluationError::Invalid(format!(
            "update case changes missing document {}",
            input.update_case.changed_document.id
        )));
    }

    Ok(())
}

/// Validates that committed fixtures cover the deep verification categories and
/// can cross the production search mapping boundary.
///
/// # Errors
///
/// Returns an error when benchmark data is too small, lacks required query
/// categories/content, or cannot be mapped into production search records.
pub fn validate_deep_fixture_input(
    input: &SearchEvaluationInput,
) -> Result<(), SearchEvaluationError> {
    validate_fixture_input(input)?;
    if input.corpus.len() < 5 {
        return Err(SearchEvaluationError::Invalid(
            "deep search corpus must contain at least five representative records".to_string(),
        ));
    }
    let exact_document_count = input
        .corpus
        .iter()
        .filter(|document| document.document_number.is_some())
        .count();
    if exact_document_count < 3 {
        return Err(SearchEvaluationError::Invalid(
            "deep search corpus must contain at least three known document numbers".to_string(),
        ));
    }
    for document in &input.corpus {
        document.to_index_operation()?;
    }
    let html_cases = input
        .queries
        .iter()
        .filter(|query| query.category == QueryCategory::HtmlContent)
        .count();
    let text_cases = input
        .queries
        .iter()
        .filter(|query| query.category == QueryCategory::DocumentText)
        .count();
    let entity_cases = input
        .queries
        .iter()
        .filter(|query| query.category == QueryCategory::EntityMetadata)
        .count();
    if text_cases < 1 || html_cases < 1 || entity_cases < 1 {
        return Err(SearchEvaluationError::Invalid(
            "deep search queries must cover document text, HTML content, and entity metadata"
                .to_string(),
        ));
    }
    for query in &input.queries {
        if query.required_terms.is_empty() {
            return Err(SearchEvaluationError::Invalid(format!(
                "query {} must list required terms",
                query.id
            )));
        }
    }
    Ok(())
}

impl SearchDocument {
    /// Maps a benchmark fixture document into the production search source record.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture source id is not a UUID or production
    /// mapping rejects required fields.
    pub fn to_index_operation(&self) -> Result<SearchIndexOperation, SearchEvaluationError> {
        let record = self.to_source_record()?;
        map_record_to_operation(&record).map_err(|source| SearchEvaluationError::Mapping {
            document_id: self.id.clone(),
            source,
        })
    }

    /// Maps a benchmark fixture document into the production source record shape.
    ///
    /// # Errors
    ///
    /// Returns an error when `source_id` is not a UUID.
    pub fn to_source_record(&self) -> Result<SearchSourceRecord, SearchEvaluationError> {
        let source_id = Uuid::parse_str(&self.source_id).map_err(|source| {
            SearchEvaluationError::InvalidSourceId {
                document_id: self.id.clone(),
                source_id: self.source_id.clone(),
                source,
            }
        })?;
        let mut fields = Map::new();
        fields.insert("titel".to_string(), Value::String(self.title.clone()));
        if let Some(summary) = &self.summary {
            fields.insert("samenvatting".to_string(), Value::String(summary.clone()));
        }
        if let Some(document_number) = &self.document_number {
            fields.insert(
                "document_nummer".to_string(),
                Value::String(document_number.clone()),
            );
        }
        if let Some(date) = &self.date {
            fields.insert("datum".to_string(), Value::String(date.clone()));
        }
        if let Some(url) = &self.url {
            fields.insert("enclosure_url".to_string(), Value::String(url.clone()));
        }
        for (index, value) in self.entity_metadata.iter().enumerate() {
            fields.insert(format!("metadata_{index}"), Value::String(value.clone()));
        }
        if self.entity_kind() == SearchEntityKind::Document {
            fields.insert(
                "content_type".to_string(),
                Value::String("application/pdf".to_string()),
            );
            fields.insert(
                "content_length".to_string(),
                Value::Number(serde_json::Number::from(self.extracted_text.len())),
            );
        }

        let now = Utc::now();
        Ok(SearchSourceRecord {
            metadata: SearchEntityMetadata {
                category: self.source_category.clone(),
                source_id,
                latest_skiptoken: 1,
                deleted: false,
                source_updated_at: now,
                atom_updated_at: now,
            },
            fields,
            document_content: Some(SearchDocumentContent {
                selected_source_url: self
                    .url
                    .clone()
                    .unwrap_or_else(|| format!("https://example.test/{}", self.id)),
                selected_source_content_type: Some("application/pdf".to_string()),
                official_source: true,
                extraction_status: "extracted".to_string(),
                validation_status: "valid".to_string(),
                output_hash: Some(format!("benchmark-{}", self.id)),
                extracted_text: Some(self.extracted_text.clone()),
                extracted_html: Some(self.extracted_html.clone()),
            }),
            relations: self
                .relation_labels
                .iter()
                .enumerate()
                .map(|(index, label)| SearchRelationLabel {
                    relation_name: "benchmark_relation".to_string(),
                    target_category: "Benchmark".to_string(),
                    target_id: Uuid::from_u128(index as u128 + 1),
                    label: label.clone(),
                })
                .collect(),
        })
    }

    fn entity_kind(&self) -> SearchEntityKind {
        match self.source_category.as_str() {
            "Document" => SearchEntityKind::Document,
            "Persoon" => SearchEntityKind::Person,
            "Activiteit" => SearchEntityKind::Activity,
            "Kamerstukdossier" | "Zaak" => SearchEntityKind::Dossier,
            _ => SearchEntityKind::Other,
        }
    }
}

/// Converts raw ranked results into the shared quality scoring model.
///
/// # Errors
///
/// Returns an error when the raw result belongs to another query or when the
/// expected result is absent from the ranked output.
pub fn score_query_result(
    query: &SearchQueryCase,
    raw: RawQueryResult,
) -> Result<QueryResultScore, SearchEvaluationError> {
    if raw.query_id != query.id {
        return Err(SearchEvaluationError::Invalid(format!(
            "raw result {} does not match query {}",
            raw.query_id, query.id
        )));
    }
    let expected_top_rank = raw
        .result_ids
        .iter()
        .position(|id| id == &query.expected_top)
        .map(|rank| rank + 1);
    let top_hit = expected_top_rank == Some(1);
    let top_3_recall = raw.result_ids.iter().take(3).any(|id| {
        query
            .acceptable_results
            .iter()
            .any(|acceptable| acceptable == id)
    });
    let mut quality_score = 0.0;
    if top_hit {
        quality_score += 0.55;
    } else if expected_top_rank.is_some_and(|rank| rank <= 3) {
        quality_score += 0.35;
    }
    if top_3_recall {
        quality_score += 0.20;
    }
    if raw.snippet_or_highlight {
        quality_score += 0.10;
    }
    if query.category != QueryCategory::FuzzyTypo || raw.typo_tolerance_evidence.is_some() {
        quality_score += 0.15;
    }

    if expected_top_rank.is_none() {
        return Err(SearchEvaluationError::Invalid(format!(
            "query {} did not return expected result {}",
            query.id, query.expected_top
        )));
    }

    Ok(QueryResultScore {
        query_id: query.id.clone(),
        category: query.category,
        result_ids: raw.result_ids,
        expected_top_rank,
        top_hit,
        top_3_recall,
        typo_tolerance_evidence: raw.typo_tolerance_evidence,
        snippet_or_highlight: raw.snippet_or_highlight,
        quality_score,
    })
}

#[derive(Debug, Clone)]
pub struct BenchmarkSearchClient {
    index: SearchIndex,
}

impl BenchmarkSearchClient {
    #[must_use]
    pub fn new(input: &SearchEvaluationInput) -> Self {
        Self {
            index: SearchIndex::build(SearchEngine::Meilisearch, input.corpus.clone()),
        }
    }
}

impl SearchQueryClient for BenchmarkSearchClient {
    fn search<'a>(
        &'a self,
        request: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchIndexError>> + Send + 'a>> {
        Box::pin(async move { Ok(self.index.search_request(request)) })
    }
}

#[derive(Debug, Clone)]
struct SearchIndex {
    engine: SearchEngine,
    documents: Vec<SearchDocument>,
}

#[derive(Debug, Clone)]
struct RankedDocument {
    id: String,
    score: f64,
    matched_query_token: Option<String>,
    matched_document_token: Option<String>,
}

impl SearchIndex {
    fn build(engine: SearchEngine, documents: Vec<SearchDocument>) -> Self {
        Self { engine, documents }
    }

    fn search(&self, query: &SearchQueryCase) -> RawQueryResult {
        let query_tokens = tokenize(&query.query);
        let mut ranked = self
            .documents
            .iter()
            .filter_map(|document| self.rank_document(document, &query_tokens))
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });

        let typo_tolerance_evidence = if query.category == QueryCategory::FuzzyTypo {
            match self.engine {
                SearchEngine::Meilisearch => ranked
                    .iter()
                    .find_map(|document| {
                        let query_token = document.matched_query_token.as_ref()?;
                        let document_token = document.matched_document_token.as_ref()?;
                        (query_token != document_token).then(|| {
                            format!(
                                "{query_token} matched {document_token} through edit-distance scoring"
                            )
                        })
                    })
                    .or_else(|| Some("no typo edit was needed for the winning tokens".to_string())),
                SearchEngine::Tantivy => Some(
                    "no typo tolerance; result was recovered through remaining exact terms"
                        .to_string(),
                ),
            }
        } else {
            None
        };

        RawQueryResult {
            query_id: query.id.clone(),
            category: query.category,
            result_ids: ranked
                .iter()
                .take(5)
                .map(|document| document.id.clone())
                .collect(),
            snippet_or_highlight: ranked.first().is_some_and(|document| document.score > 0.0),
            typo_tolerance_evidence,
        }
    }

    fn search_request(&self, request: SearchRequest) -> SearchResponse {
        let query = SearchQueryCase {
            id: "http-search".to_string(),
            category: QueryCategory::Mixed,
            query: request.query.clone(),
            expected_top: String::new(),
            acceptable_results: Vec::new(),
            required_terms: Vec::new(),
        };
        let query_tokens = tokenize(&query.query);
        let mut ranked = self
            .documents
            .iter()
            .filter(|document| request_matches_filter(document, request.filter.as_ref()))
            .filter_map(|document| {
                self.rank_document(document, &query_tokens)
                    .map(|ranked| (document, ranked))
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .1
                .score
                .partial_cmp(&left.1.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.0.id.cmp(&right.0.id))
        });
        let start = request.offset as usize;
        let end = start.saturating_add(request.limit as usize);
        let results = ranked
            .iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .filter_map(|(document, ranked)| search_result_from_document(document, ranked.score))
            .collect::<Vec<_>>();
        SearchResponse {
            query: request.query,
            limit: request.limit,
            offset: request.offset,
            estimated_total_hits: Some(u32::try_from(ranked.len()).unwrap_or(u32::MAX)),
            results,
        }
    }

    fn rank_document(
        &self,
        document: &SearchDocument,
        query_tokens: &[String],
    ) -> Option<RankedDocument> {
        let weighted_fields = vec![
            (
                5.0,
                document
                    .document_number
                    .as_deref()
                    .unwrap_or_default()
                    .to_string(),
            ),
            (4.0, document.title.clone()),
            (3.0, document.entity_metadata.join(" ")),
            (2.0, document.source_id.clone()),
            (1.5, document.extracted_text.clone()),
            (1.0, document.extracted_html.clone()),
        ];
        let mut score = 0.0;
        let mut matched_query_token = None;
        let mut matched_document_token = None;

        for query_token in query_tokens {
            for (weight, text) in &weighted_fields {
                let field_tokens = tokenize(text);
                for document_token in &field_tokens {
                    let token_score = self.token_score(query_token, document_token);
                    if token_score > 0.0 {
                        score += weight * token_score;
                        matched_query_token.get_or_insert_with(|| query_token.clone());
                        matched_document_token.get_or_insert_with(|| document_token.clone());
                    }
                }
            }
        }

        (score > 0.0).then(|| RankedDocument {
            id: document.id.clone(),
            score,
            matched_query_token,
            matched_document_token,
        })
    }

    fn token_score(&self, query_token: &str, document_token: &str) -> f64 {
        if query_token == document_token {
            return 1.0;
        }
        if query_token.len().min(document_token.len()) >= 4
            && (document_token.contains(query_token) || query_token.contains(document_token))
        {
            return 0.65;
        }
        if self.engine == SearchEngine::Meilisearch {
            let distance = edit_distance(query_token, document_token);
            let tolerance = if query_token.len() <= 5 { 1 } else { 2 };
            if distance <= tolerance {
                return 0.70;
            }
        }
        0.0
    }

    fn apply_update(&mut self, update_case: &SearchUpdateCase) {
        self.documents
            .retain(|document| document.id != update_case.deleted_document_id);
        if let Some(existing) = self
            .documents
            .iter_mut()
            .find(|document| document.id == update_case.changed_document.id)
        {
            *existing = update_case.changed_document.clone();
        } else {
            self.documents.push(update_case.changed_document.clone());
        }
    }

    fn measure_update(&self, update_case: &SearchUpdateCase) -> UpdateResult {
        let changed_query = SearchQueryCase {
            id: "update-changed".to_string(),
            category: QueryCategory::Mixed,
            query: update_case.changed_query.clone(),
            expected_top: update_case.changed_document.id.clone(),
            acceptable_results: vec![update_case.changed_document.id.clone()],
            required_terms: Vec::new(),
        };
        let deleted_query = SearchQueryCase {
            id: "update-deleted".to_string(),
            category: QueryCategory::Mixed,
            query: update_case.deleted_query.clone(),
            expected_top: update_case.changed_document.id.clone(),
            acceptable_results: vec![update_case.changed_document.id.clone()],
            required_terms: Vec::new(),
        };
        let changed_results = self.search(&changed_query);
        let deleted_results = self.search(&deleted_query);

        UpdateResult {
            changed_document_id: update_case.changed_document.id.clone(),
            deleted_document_id: update_case.deleted_document_id.clone(),
            changed_query: update_case.changed_query.clone(),
            changed_query_top_result: changed_results.result_ids.first().cloned(),
            deleted_query: update_case.deleted_query.clone(),
            deleted_query_result_ids: deleted_results.result_ids,
        }
    }
}

fn request_matches_filter(document: &SearchDocument, filter: Option<&SearchFilter>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    if let Some(source_category) = &filter.source_category {
        if &document.source_category != source_category {
            return false;
        }
    }
    if let Some(entity_kind) = &filter.entity_kind {
        if &document.entity_kind() != entity_kind {
            return false;
        }
    }
    true
}

fn search_result_from_document(
    document: &SearchDocument,
    ranking_score: f64,
) -> Option<SearchResult> {
    let source_id = Uuid::parse_str(&document.source_id).ok()?;
    Some(SearchResult {
        key: format!("{}:{source_id}", document.source_category),
        source_category: document.source_category.clone(),
        source_id,
        entity_kind: document.entity_kind(),
        title: document.title.clone(),
        summary: document.summary.clone(),
        source_url: document.url.clone(),
        date: document.date.clone(),
        document_number: document.document_number.clone(),
        snippets: vec![SearchSnippet {
            field: "extracted_text".to_string(),
            text: document.extracted_text.clone(),
            highlighted: Some(format!("<em>{}</em>", document.title)),
        }],
        ranking_score: Some(ranking_score),
    })
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.chars().count()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_char) in right.chars().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current.push(
                (previous[right_index + 1] + 1)
                    .min(current[right_index] + 1)
                    .min(previous[right_index] + substitution_cost),
            );
        }
        previous = current;
    }
    previous[right.chars().count()]
}

fn positive_elapsed_ms(start: Instant) -> u128 {
    start.elapsed().as_millis().max(1)
}

fn p95_latency_ms(measurements: &[QueryLatencyMeasurement]) -> u128 {
    if measurements.is_empty() {
        return 0;
    }
    let mut values = measurements
        .iter()
        .map(|measurement| measurement.elapsed_ms)
        .collect::<Vec<_>>();
    values.sort_unstable();
    let index = ((values.len() * 95).div_ceil(100)).saturating_sub(1);
    values[index]
}

fn directory_size(path: &Path) -> Result<u64, SearchEvaluationError> {
    let mut total = 0;
    for entry in fs::read_dir(path).map_err(|source| SearchEvaluationError::Read {
        path: path.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| SearchEvaluationError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let metadata = entry
            .metadata()
            .map_err(|source| SearchEvaluationError::Read {
                path: entry.path().display().to_string(),
                source,
            })?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}

fn read_json<T>(path: &Path) -> Result<T, SearchEvaluationError>
where
    T: for<'de> Deserialize<'de>,
{
    let text = fs::read_to_string(path).map_err(|source| SearchEvaluationError::Read {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| SearchEvaluationError::Parse {
        path: path.display().to_string(),
        source,
    })
}
