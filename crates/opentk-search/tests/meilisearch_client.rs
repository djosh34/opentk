use std::{
    io::ErrorKind,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use chrono::{TimeZone, Utc};
use opentk_search::{
    meilisearch_schema, MeilisearchClient, SearchCountClient, SearchEntityKind, SearchFilter,
    SearchHealthClient, SearchIndexClient, SearchIndexDocument, SearchIndexOperation,
    SearchQueryClient, SearchRequest,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use uuid::Uuid;

#[tokio::test]
async fn meilisearch_client_resets_settings_upserts_deletes_and_polls_tasks() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
    let address = listener.local_addr().expect("server address");
    let server_observed = Arc::clone(&observed);
    let next_task = Arc::new(AtomicU64::new(1));
    let server_next_task = Arc::clone(&next_task);
    let server = tokio::spawn(async move {
        serve_meili_fixture(listener, server_observed, server_next_task).await;
    });

    let client = MeilisearchClient::new(
        format!("http://{address}"),
        Some("secret".to_owned()),
        "opentk_entities".to_owned(),
    );
    client
        .reset_index(&meilisearch_schema())
        .await
        .expect("reset succeeds");
    client
        .apply_batch(&[
            SearchIndexOperation::Upsert(Box::new(search_document())),
            SearchIndexOperation::Delete("Document_deleted".to_owned()),
        ])
        .await
        .expect("batch succeeds");

    server.abort();
    let _ = server.await;
    let observed = observed.lock().expect("observed mutex");
    assert!(observed.iter().any(|request| {
        let lower = request.to_ascii_lowercase();
        request.starts_with("DELETE /indexes/opentk_entities ")
            && lower.contains("authorization: bearer secret")
    }));
    assert!(observed
        .iter()
        .any(|request| request.starts_with("POST /indexes ")
            && request.contains("\"primaryKey\":\"id\"")));
    assert!(observed.iter().any(|request| request
        .starts_with("PUT /indexes/opentk_entities/settings/searchable-attributes ")));
    assert!(observed.iter().any(|request| request
        .starts_with("PUT /indexes/opentk_entities/settings/pagination ")
        && request.contains("\"maxTotalHits\":10000000")));
    assert!(observed.iter().any(|request| request
        .starts_with("POST /indexes/opentk_entities/documents ")
        && request.contains("\"id\":\"Document_indexed\"")
        && request.contains("\"key\":\"Document:indexed\"")
        && request.contains("\"title\":\"Indexed document\"")));
    assert!(observed.iter().any(|request| request
        .starts_with("POST /indexes/opentk_entities/documents/delete-batch ")
        && request.contains("Document_deleted")));
    assert!(observed
        .iter()
        .any(|request| request.starts_with("GET /tasks/")));
}

#[tokio::test]
async fn meilisearch_client_searches_with_fuzzy_options_and_maps_results() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
    let address = listener.local_addr().expect("server address");
    let server_observed = Arc::clone(&observed);
    let server = tokio::spawn(async move {
        serve_meili_search_fixture(listener, server_observed).await;
    });

    let client = MeilisearchClient::new(
        format!("http://{address}"),
        Some("secret".to_owned()),
        "opentk_entities".to_owned(),
    );
    let response = client
        .search(SearchRequest {
            query: "kamerbriev".to_owned(),
            limit: 2,
            offset: 1,
            filter: None,
        })
        .await
        .expect("search succeeds");

    server.abort();
    let _ = server.await;
    let observed = observed.lock().expect("observed mutex");
    let request = observed
        .iter()
        .find(|request| request.starts_with("POST /indexes/opentk_entities/search "))
        .expect("search request observed");
    assert!(request.contains("\"q\":\"kamerbriev\""));
    assert!(request.contains("\"limit\":2"));
    assert!(request.contains("\"offset\":1"));
    assert!(request.contains("\"attributesToHighlight\""));
    assert!(request.contains("\"attributesToCrop\""));
    assert!(request.contains("\"showRankingScore\":true"));

    assert_eq!(response.query, "kamerbriev");
    assert_eq!(response.limit, 2);
    assert_eq!(response.offset, 1);
    assert_eq!(response.estimated_total_hits, Some(1));
    assert_eq!(response.results.len(), 1);
    let result = &response.results[0];
    assert_eq!(result.title, "Fixture document");
    assert_eq!(result.ranking_score, Some(0.98));
    assert_eq!(result.snippets.len(), 1);
    assert_eq!(result.snippets[0].field, "extracted_text");
    assert_eq!(result.snippets[0].text, "plain snippet");
    assert_eq!(
        result.snippets[0].highlighted.as_deref(),
        Some("plain <em>snippet</em>")
    );
    assert!(
        result
            .snippets
            .iter()
            .all(|snippet| snippet.field != "relation_labels"),
        "internal relation labels must not be exposed as public snippets"
    );
}

#[tokio::test]
async fn meilisearch_client_counts_filtered_documents() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
    let address = listener.local_addr().expect("server address");
    let server_observed = Arc::clone(&observed);
    let server = tokio::spawn(async move {
        serve_meili_search_fixture(listener, server_observed).await;
    });

    let client = MeilisearchClient::new(
        format!("http://{address}"),
        Some("secret".to_owned()),
        "opentk_entities".to_owned(),
    );
    let count = client
        .count(SearchFilter {
            source_category: Some("Document".to_owned()),
            entity_kind: None,
        })
        .await
        .expect("count succeeds");

    server.abort();
    let _ = server.await;
    let observed = observed.lock().expect("observed mutex");
    let request = observed
        .iter()
        .find(|request| request.starts_with("POST /indexes/opentk_entities/search "))
        .expect("count search request observed");
    assert!(request.contains("\"limit\":0"));
    assert!(request.contains("source_category = \\\"Document\\\""));
    assert_eq!(count, 1);
}

#[tokio::test]
async fn meilisearch_client_health_uses_authenticated_stats_request() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
    let address = listener.local_addr().expect("server address");
    let server_observed = Arc::clone(&observed);
    let server = tokio::spawn(async move {
        serve_meili_health_fixture(listener, server_observed).await;
    });

    let client = MeilisearchClient::new(
        format!("http://{address}"),
        Some("secret".to_owned()),
        "opentk_entities".to_owned(),
    );
    client.health().await.expect("health succeeds");

    server.abort();
    let _ = server.await;
    let observed = observed.lock().expect("observed mutex");
    assert!(observed.iter().any(|request| {
        let lower = request.to_ascii_lowercase();
        request.starts_with("GET /stats ") && lower.contains("authorization: bearer secret")
    }));
}

async fn serve_meili_fixture(
    listener: TcpListener,
    observed: Arc<Mutex<Vec<String>>>,
    next_task: Arc<AtomicU64>,
) {
    loop {
        let stream = listener.accept().await;
        let Ok((mut stream, _)) = stream else {
            return;
        };
        let observed = Arc::clone(&observed);
        let next_task = Arc::clone(&next_task);
        tokio::spawn(async move {
            let mut buffer = vec![0_u8; 16 * 1024];
            let read = match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => return,
                Ok(read) => read,
            };
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            observed
                .lock()
                .expect("observed mutex")
                .push(request.clone());
            let first_line = request.lines().next().unwrap_or_default();
            let body = if first_line.starts_with("GET /tasks/") {
                r#"{"status":"succeeded"}"#.to_owned()
            } else {
                let task = next_task.fetch_add(1, Ordering::Relaxed);
                format!(r#"{{"taskUid":{task}}}"#)
            };
            let status = if first_line.starts_with("DELETE /indexes/") {
                "202 Accepted"
            } else {
                "200 OK"
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if let Err(error) = stream.write_all(response.as_bytes()).await {
                assert_eq!(error.kind(), ErrorKind::BrokenPipe);
            }
        });
    }
}

async fn serve_meili_search_fixture(listener: TcpListener, observed: Arc<Mutex<Vec<String>>>) {
    loop {
        let stream = listener.accept().await;
        let Ok((mut stream, _)) = stream else {
            return;
        };
        let observed = Arc::clone(&observed);
        tokio::spawn(async move {
            let mut buffer = vec![0_u8; 16 * 1024];
            let read = match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => return,
                Ok(read) => read,
            };
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            observed
                .lock()
                .expect("observed mutex")
                .push(request.clone());
            let first_line = request.lines().next().unwrap_or_default();
            let (status, body) = if first_line.starts_with("POST /indexes/opentk_entities/search ")
            {
                (
                    "200 OK",
                    r#"{"hits":[{"key":"Document:11111111-1111-4111-8111-111111111111","source_category":"Document","source_id":"11111111-1111-4111-8111-111111111111","entity_kind":"Document","title":"Fixture document","summary":"Read endpoint","source_url":"https://example.test/document.pdf","date":"2026-04-26T00:00:00Z","document_number":"2026D00001","_formatted":{"extracted_text":"plain <em>snippet</em>","relation_labels":["persoon Persoon 2404","persoon Persoon 2404"]},"_rankingScore":0.98}],"estimatedTotalHits":1}"#,
                )
            } else {
                ("404 Not Found", r#"{"message":"unexpected request"}"#)
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if let Err(error) = stream.write_all(response.as_bytes()).await {
                assert_eq!(error.kind(), ErrorKind::BrokenPipe);
            }
        });
    }
}

async fn serve_meili_health_fixture(listener: TcpListener, observed: Arc<Mutex<Vec<String>>>) {
    loop {
        let stream = listener.accept().await;
        let Ok((mut stream, _)) = stream else {
            return;
        };
        let observed = Arc::clone(&observed);
        tokio::spawn(async move {
            let mut buffer = vec![0_u8; 16 * 1024];
            let read = match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => return,
                Ok(read) => read,
            };
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            observed.lock().expect("observed mutex").push(request);
            let body = r#"{"databaseSize":0}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if let Err(error) = stream.write_all(response.as_bytes()).await {
                assert_eq!(error.kind(), ErrorKind::BrokenPipe);
            }
        });
    }
}

fn search_document() -> SearchIndexDocument {
    SearchIndexDocument {
        id: "Document_indexed".to_owned(),
        key: "Document:indexed".to_owned(),
        source_category: "Document".to_owned(),
        source_id: Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("uuid"),
        entity_kind: SearchEntityKind::Document,
        title: "Indexed document".to_owned(),
        summary: None,
        source_url: Some("https://example.test/document".to_owned()),
        date: Some("2026-04-26".to_owned()),
        document_number: Some("2026D00001".to_owned()),
        extracted_text: Some("text".to_owned()),
        extracted_html: Some("<p>html</p>".to_owned()),
        metadata_text: vec!["kamer: 2".to_owned()],
        relation_labels: vec!["zaak Zaak 123".to_owned()],
        filter_categories: vec!["Document".to_owned()],
        latest_skiptoken: 42,
        source_updated_at: Utc.with_ymd_and_hms(2026, 4, 26, 0, 0, 0).unwrap(),
        atom_updated_at: Utc.with_ymd_and_hms(2026, 4, 26, 0, 1, 0).unwrap(),
    }
}
