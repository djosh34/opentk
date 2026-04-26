use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub source_category: String,
    pub source_id: String,
    pub document_number: Option<String>,
    pub date: Option<String>,
    pub url: Option<String>,
    pub extracted_text: String,
    pub extracted_html: String,
    pub entity_metadata: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQueryCase {
    pub id: String,
    pub category: QueryCategory,
    pub query: String,
    pub expected_top: String,
    pub acceptable_results: Vec<String>,
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
        };
        let deleted_query = SearchQueryCase {
            id: "update-deleted".to_string(),
            category: QueryCategory::Mixed,
            query: update_case.deleted_query.clone(),
            expected_top: update_case.changed_document.id.clone(),
            acceptable_results: vec![update_case.changed_document.id.clone()],
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
