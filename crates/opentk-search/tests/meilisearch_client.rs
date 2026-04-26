use std::{
    io::ErrorKind,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use chrono::{TimeZone, Utc};
use opentk_search::{
    meilisearch_schema, MeilisearchClient, SearchEntityKind, SearchIndexClient,
    SearchIndexDocument, SearchIndexOperation,
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
            SearchIndexOperation::Delete("Document:deleted".to_owned()),
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
        .any(|request| request.starts_with("POST /indexes ")));
    assert!(observed.iter().any(|request| request
        .starts_with("PUT /indexes/opentk_entities/settings/searchable-attributes ")));
    assert!(observed.iter().any(|request| request
        .starts_with("POST /indexes/opentk_entities/documents ")
        && request.contains("\"title\":\"Indexed document\"")));
    assert!(observed.iter().any(|request| request
        .starts_with("POST /indexes/opentk_entities/documents/delete-batch ")
        && request.contains("Document:deleted")));
    assert!(observed
        .iter()
        .any(|request| request.starts_with("GET /tasks/")));
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

fn search_document() -> SearchIndexDocument {
    SearchIndexDocument {
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
