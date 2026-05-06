use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use opentk_config::{Config, ConfigLoader, EffectiveLogConfig, LogConfig, LogFormat};
use serde_json::Value;

#[test]
fn required_database_url_loads_with_compiled_defaults() {
    let config = Config::from_toml_str(
        r#"
        [database]
        url = "postgres://postgres:postgres@postgres:5432/opentk"
        "#,
    )
    .expect("minimal config loads");

    assert_eq!(
        config.database.url,
        "postgres://postgres:postgres@postgres:5432/opentk"
    );
    assert_eq!(config.database.max_connections, 5);
    assert_eq!(
        config.sync.base_url.as_str(),
        "https://gegevensmagazijn.tweedekamer.nl/"
    );
    assert_eq!(config.sync.request_timeout_secs, 30);
    assert_eq!(config.sync.connect_timeout_secs, 10);
    assert_eq!(config.sync.max_retries, 3);
    assert_eq!(config.sync.max_concurrent_requests.get(), 4);
    assert_eq!(config.sync.poll_interval_secs, 30);
    assert!(config.sync.categories.contains(&"Document".to_owned()));
    assert!(config.sync.categories.contains(&"Zaak".to_owned()));
    assert_eq!(config.search.url, "http://meilisearch:7700");
    assert_eq!(config.search.api_key, None);
    assert_eq!(config.search.index_name, "opentk_entities");
    assert_eq!(config.search.batch_size, 100);
    assert_eq!(config.search.max_payload_bytes, 80_000_000);
    assert_eq!(config.search.retry_limit, 3);
    assert_eq!(config.api.bind_address.to_string(), "0.0.0.0:3000");
    assert!(config.api.cors_origins.is_empty());
    assert_eq!(config.api.max_public_query_limit, 1000);
    assert_eq!(config.log.format, LogFormat::Pretty);
    assert_eq!(config.log.level, "info");
}

#[test]
fn explicit_path_loads_file_and_missing_explicit_path_is_clear_error() {
    let temp = TempDir::new();
    let config_path = temp.path().join("selected.toml");
    fs::write(
        &config_path,
        r#"
        [database]
        url = "postgres://selected.example.test/opentk"
        "#,
    )
    .expect("write config file");

    let loaded = ConfigLoader::with_path(&config_path)
        .load()
        .expect("explicit path loads");

    assert_eq!(loaded.path, config_path);
    assert_eq!(
        loaded.config.database.url,
        "postgres://selected.example.test/opentk"
    );

    let missing = temp.path().join("missing.toml");
    let error = ConfigLoader::with_path(&missing)
        .load()
        .expect_err("missing explicit file fails");
    assert!(error
        .to_string()
        .contains(missing.to_str().expect("utf8 path")));
    assert!(error.to_string().contains("does not exist"));
}

#[test]
fn invalid_config_reports_field_context() {
    for (toml, field) in [
        ("", "database.url"),
        ("[database]\nurl = ''", "database.url"),
        (
            "[database]\nurl = 'postgres://example.test/db'\nmax_connections = 0",
            "database.max_connections",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[sync]\nbase_url = 'not a url'",
            "sync.base_url",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[sync]\nmax_concurrent_requests = 0",
            "sync.max_concurrent_requests",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[search]\nbatch_size = 0",
            "search.batch_size",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[search]\nmax_payload_bytes = 0",
            "search.max_payload_bytes",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[search]\nretry_limit = 0",
            "search.retry_limit",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[api]\nbind_address = 'not a socket'",
            "api.bind_address",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[api]\nmax_public_query_limit = 0",
            "api.max_public_query_limit",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[log]\nformat = 'xml'",
            "format",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\nunknown = true",
            "unknown",
        ),
    ] {
        let error = Config::from_toml_str(toml).expect_err("config should fail");
        assert!(
            error.to_string().contains(field),
            "expected {field:?} in {error}"
        );
    }
}

#[test]
fn explicit_values_override_defaults() {
    let config = Config::from_toml_str(
        r#"
        [database]
        url = "postgres://postgres:postgres@db:5432/opentk"
        max_connections = 9

        [sync]
        base_url = "https://sync.example.test"
        request_timeout_secs = 11
        connect_timeout_secs = 12
        max_retries = 13
        max_concurrent_requests = 14
        poll_interval_secs = 15
        categories = ["Document"]

        [search]
        url = "http://search.example.test:7700"
        api_key = "secret"
        index_name = "custom_index"
        batch_size = 16
        max_payload_bytes = 12345
        retry_limit = 17

        [api]
        bind_address = "127.0.0.1:3001"
        cors_origins = ["https://app.example.test"]
        max_public_query_limit = 2500

        [log]
        format = "json"
        level = "debug"
        "#,
    )
    .expect("full config loads");

    assert_eq!(config.database.max_connections, 9);
    assert_eq!(config.sync.base_url.as_str(), "https://sync.example.test/");
    assert_eq!(config.sync.request_timeout_secs, 11);
    assert_eq!(config.sync.connect_timeout_secs, 12);
    assert_eq!(config.sync.max_retries, 13);
    assert_eq!(config.sync.max_concurrent_requests.get(), 14);
    assert_eq!(config.sync.poll_interval_secs, 15);
    assert_eq!(config.sync.categories, ["Document"]);
    assert_eq!(config.search.url, "http://search.example.test:7700");
    assert_eq!(config.search.api_key.as_deref(), Some("secret"));
    assert_eq!(config.search.index_name, "custom_index");
    assert_eq!(config.search.batch_size, 16);
    assert_eq!(config.search.max_payload_bytes, 12345);
    assert_eq!(config.search.retry_limit, 17);
    assert_eq!(config.api.bind_address.to_string(), "127.0.0.1:3001");
    assert_eq!(config.api.cors_origins, ["https://app.example.test"]);
    assert_eq!(config.api.max_public_query_limit, 2500);
    assert_eq!(config.log.format, LogFormat::Json);
    assert_eq!(config.log.level, "debug");
}

#[test]
fn redacted_config_masks_secrets_and_keeps_operational_settings() {
    let config = Config::from_toml_str(
        r#"
        [database]
        url = "postgres://postgres:secret@db:5432/opentk"
        max_connections = 9

        [sync]
        base_url = "https://sync.example.test"
        request_timeout_secs = 11
        connect_timeout_secs = 12
        max_retries = 13
        max_concurrent_requests = 14
        poll_interval_secs = 15
        categories = ["Document"]

        [search]
        url = "http://search.example.test:7700"
        api_key = "search-secret"
        index_name = "custom_index"
        batch_size = 16
        max_payload_bytes = 12345
        retry_limit = 17

        [api]
        bind_address = "127.0.0.1:3001"
        cors_origins = ["https://app.example.test"]
        max_public_query_limit = 2500

        [log]
        format = "json"
        level = "debug"
        "#,
    )
    .expect("full config loads");

    let redacted = serde_json::to_value(config.redacted()).expect("redacted config serializes");

    assert_eq!(
        redacted["database"],
        serde_json::json!({ "max_connections": 9 })
    );
    assert_eq!(
        redacted["search"]["api_key"],
        Value::String("***".to_owned())
    );
    assert_eq!(redacted["search"]["url"], "http://search.example.test:7700");
    assert_eq!(redacted["search"]["index_name"], "custom_index");
    assert_eq!(redacted["search"]["batch_size"], 16);
    assert_eq!(redacted["search"]["max_payload_bytes"], 12345);
    assert_eq!(redacted["search"]["retry_limit"], 17);
    assert_eq!(redacted["sync"]["base_url"], "https://sync.example.test/");
    assert_eq!(redacted["api"]["bind_address"], "127.0.0.1:3001");
    assert_eq!(redacted["api"]["max_public_query_limit"], 2500);
    assert_eq!(redacted["log"]["level"], "debug");
    assert!(
        !redacted.to_string().contains("postgres://"),
        "database URL must not be present in redacted config"
    );
    assert!(
        !redacted.to_string().contains("search-secret"),
        "search API key must not be present in redacted config"
    );
}

#[test]
fn effective_log_config_uses_toml_defaults_without_env_overrides() {
    let log = LogConfig {
        format: LogFormat::Json,
        level: "info,sqlx=warn".to_owned(),
    };

    let effective = EffectiveLogConfig::from_overrides(&log, None, None)
        .expect("config logging settings resolve");

    assert_eq!(
        effective,
        EffectiveLogConfig {
            format: LogFormat::Json,
            level: "info,sqlx=warn".to_owned(),
        }
    );
}

#[test]
fn effective_log_config_accepts_deployment_env_overrides() {
    let log = LogConfig {
        format: LogFormat::Pretty,
        level: "info".to_owned(),
    };

    let effective = EffectiveLogConfig::from_overrides(
        &log,
        Some("json"),
        Some("info,opentk_api=debug,sqlx=warn"),
    )
    .expect("env overrides resolve");

    assert_eq!(
        effective,
        EffectiveLogConfig {
            format: LogFormat::Json,
            level: "info,opentk_api=debug,sqlx=warn".to_owned(),
        }
    );
}

#[test]
fn effective_log_config_rejects_unknown_format_override() {
    let log = LogConfig {
        format: LogFormat::Pretty,
        level: "info".to_owned(),
    };

    let error = EffectiveLogConfig::from_overrides(&log, Some("xml"), None)
        .expect_err("unsupported log format fails");

    assert!(error.to_string().contains("OPENTK_LOG_FORMAT"));
    assert!(error.to_string().contains("json or pretty"));
}

#[test]
fn compose_config_uses_service_hostnames_and_explicit_sync_scope() {
    let loaded = ConfigLoader::with_path(workspace_path("config/opentk.compose.toml"))
        .load()
        .expect("compose config should load");

    assert_eq!(
        loaded.config.database.url,
        "postgres://opentk:opentk@postgres:5432/opentk"
    );
    assert_eq!(loaded.config.search.url, "http://meilisearch:7700");
    assert_eq!(loaded.config.api.bind_address.to_string(), "0.0.0.0:3000");
    for category in ["Document", "Zaak", "Activiteit", "Stemming"] {
        assert!(
            loaded
                .config
                .sync
                .categories
                .iter()
                .any(|loaded| loaded == category),
            "compose sync categories should include {category}"
        );
    }
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        path.push(format!("opentk-config-test-{suffix}"));
        fs::create_dir(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).expect("remove temp dir");
    }
}

fn workspace_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}
