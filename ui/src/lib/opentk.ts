export const API_BASE_URL = "https://api.watdoenzedaar.nl";

export type SearchResult = {
  key: string;
  source_category: string;
  entity_kind: string;
  title: string;
  summary: string | null;
  source_url: string | null;
  api_url: string;
  date: string | null;
  document_number: string | null;
  snippets: Array<{ field: string; text: string; highlighted: string | null }>;
};

export type SearchResponse = {
  query: string;
  estimated_total_hits: number | null;
  items: SearchResult[];
};

export type EntityDetail = {
  metadata: {
    category: string;
    source_id: string;
    latest_skiptoken: number;
    deleted: boolean;
    source_updated_at: string | null;
    atom_updated_at: string | null;
  };
  fields: Record<string, unknown>;
  relations?: RelationDetail[];
};

export type RelationDetail = {
  source_category: string;
  source_id: string;
  relation_name: string;
  target_category: string;
  target_id: string;
  ordinal: number | null;
};

export type DocumentContent = {
  content: {
    selected_source_url: string;
    selected_source_content_type: string | null;
    selected_source_content_length: number | null;
    official_source: boolean;
    extraction_status: string;
    validation_status: string;
    extraction_tool: string;
    extraction_tool_version: string;
    extraction_error: string | null;
    extracted_text: string | null;
    extracted_html: string | null;
    extracted_at: string | null;
  };
};

export async function search(query: string, signal: AbortSignal): Promise<SearchResponse> {
  const url = apiUrl("/search");
  url.searchParams.set("q", query);
  url.searchParams.set("limit", "20");
  return fetchJson<SearchResponse>(url, signal, "Search");
}

export async function entityDetail(apiPath: string, signal: AbortSignal): Promise<EntityDetail> {
  const url = apiUrl(apiPath);
  url.searchParams.set("relations", "both");
  return fetchJson<EntityDetail>(url, signal, "Detail");
}

export async function documentContent(apiPath: string, signal: AbortSignal): Promise<DocumentContent | null> {
  const response = await fetch(apiUrl(`${apiPath}/content`), {
    headers: { Accept: "application/json" },
    signal,
  });
  if (response.status === 404) {
    return null;
  }
  if (!response.ok) {
    throw new Error(`Document content failed with HTTP ${response.status}`);
  }
  return response.json() as Promise<DocumentContent>;
}

export function apiUrl(path: string): URL {
  return new URL(path, API_BASE_URL);
}

async function fetchJson<T>(url: URL, signal: AbortSignal, label: string): Promise<T> {
  const response = await fetch(url, {
    headers: { Accept: "application/json" },
    signal,
  });
  if (!response.ok) {
    throw new Error(`${label} failed with HTTP ${response.status}`);
  }
  return response.json() as Promise<T>;
}
