use axum::{
    extract::{rejection::QueryRejection, Query, State},
    Json,
};
use opentk_search::{
    SearchEntityKind, SearchFilter, SearchRequest, SearchResponse, SearchResult, SearchSnippet,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{ensure_search_sync_usable, ApiError, ApiState};

const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 100;

#[derive(Debug, Deserialize)]
pub(crate) struct SearchQuery {
    q: String,
    limit: Option<u32>,
    offset: Option<u32>,
    category: Option<String>,
    entity_kind: Option<SearchEntityKind>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SearchResponseDto {
    query: String,
    limit: u32,
    offset: u32,
    estimated_total_hits: Option<u32>,
    items: Vec<SearchResultDto>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SearchResultDto {
    key: String,
    source_category: String,
    source_id: String,
    entity_kind: SearchEntityKindDto,
    title: String,
    summary: Option<String>,
    source_url: Option<String>,
    api_url: String,
    date: Option<String>,
    document_number: Option<String>,
    snippets: Vec<SearchSnippetDto>,
    ranking_score: Option<f64>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SearchSnippetDto {
    field: String,
    text: String,
    highlighted: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) enum SearchEntityKindDto {
    Document,
    Person,
    Activity,
    Dossier,
    Other,
}

pub(crate) async fn search(
    State(state): State<ApiState>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<SearchResponseDto>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let request = query.into_request()?;
    ensure_search_sync_usable(&state)?;
    let response = state.search.search(request).await?;
    Ok(Json(search_response(response)))
}

impl SearchQuery {
    fn into_request(self) -> Result<SearchRequest, ApiError> {
        let query = self.q.trim().to_owned();
        if query.is_empty() {
            return Err(ApiError::InvalidRequest);
        }
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(ApiError::InvalidRequest);
        }

        Ok(SearchRequest {
            query,
            limit,
            offset: self.offset.unwrap_or(0),
            filter: search_filter(self.category, self.entity_kind),
        })
    }
}

fn search_filter(
    source_category: Option<String>,
    entity_kind: Option<SearchEntityKind>,
) -> Option<SearchFilter> {
    let source_category = source_category
        .map(|category| category.trim().to_owned())
        .filter(|category| !category.is_empty());
    if source_category.is_none() && entity_kind.is_none() {
        None
    } else {
        Some(SearchFilter {
            source_category,
            entity_kind,
        })
    }
}

fn search_response(response: SearchResponse) -> SearchResponseDto {
    SearchResponseDto {
        query: response.query,
        limit: response.limit,
        offset: response.offset,
        estimated_total_hits: response.estimated_total_hits,
        items: response.results.into_iter().map(search_result).collect(),
    }
}

fn search_result(result: SearchResult) -> SearchResultDto {
    let api_url = api_url(&result);
    SearchResultDto {
        key: result.key,
        source_category: result.source_category,
        source_id: result.source_id.to_string(),
        entity_kind: entity_kind(&result.entity_kind),
        title: result.title,
        summary: result.summary,
        source_url: result.source_url,
        api_url,
        date: result.date,
        document_number: result.document_number,
        snippets: result.snippets.into_iter().map(search_snippet).collect(),
        ranking_score: result.ranking_score,
    }
}

fn search_snippet(snippet: SearchSnippet) -> SearchSnippetDto {
    SearchSnippetDto {
        field: snippet.field,
        text: snippet.text,
        highlighted: snippet.highlighted,
    }
}

fn entity_kind(entity_kind: &SearchEntityKind) -> SearchEntityKindDto {
    match entity_kind {
        SearchEntityKind::Document => SearchEntityKindDto::Document,
        SearchEntityKind::Person => SearchEntityKindDto::Person,
        SearchEntityKind::Activity => SearchEntityKindDto::Activity,
        SearchEntityKind::Dossier => SearchEntityKindDto::Dossier,
        SearchEntityKind::Other => SearchEntityKindDto::Other,
    }
}

fn api_url(result: &SearchResult) -> String {
    match result.entity_kind {
        SearchEntityKind::Document => format!("/documents/{}", result.source_id),
        SearchEntityKind::Person => format!("/persons/{}", result.source_id),
        SearchEntityKind::Activity => format!("/activities/{}", result.source_id),
        SearchEntityKind::Dossier | SearchEntityKind::Other => {
            format!("/entities/{}/{}", result.source_category, result.source_id)
        }
    }
}
