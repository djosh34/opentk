use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use opentk_config::{Config, ConfigLoader, LogFormat};
use serde_json::Value;
use serde_yaml::Value as YamlValue;

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
    assert_eq!(config.search.retry_limit, 3);
    assert_eq!(config.api.bind_address.to_string(), "0.0.0.0:3000");
    assert!(config.api.cors_origins.is_empty());
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
            "[database]\nurl = 'postgres://example.test/db'\n[search]\nretry_limit = 0",
            "search.retry_limit",
        ),
        (
            "[database]\nurl = 'postgres://example.test/db'\n[api]\nbind_address = 'not a socket'",
            "api.bind_address",
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
        retry_limit = 17

        [api]
        bind_address = "127.0.0.1:3001"
        cors_origins = ["https://app.example.test"]

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
    assert_eq!(config.search.retry_limit, 17);
    assert_eq!(config.api.bind_address.to_string(), "127.0.0.1:3001");
    assert_eq!(config.api.cors_origins, ["https://app.example.test"]);
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
        retry_limit = 17

        [api]
        bind_address = "127.0.0.1:3001"
        cors_origins = ["https://app.example.test"]

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
    assert_eq!(redacted["search"]["retry_limit"], 17);
    assert_eq!(redacted["sync"]["base_url"], "https://sync.example.test/");
    assert_eq!(redacted["api"]["bind_address"], "127.0.0.1:3001");
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
fn docker_compose_declares_local_stack_contract() {
    let compose = fs::read_to_string(workspace_path("docker-compose.yml"))
        .expect("docker-compose.yml should be readable");
    let compose: YamlValue = serde_yaml::from_str(&compose).expect("compose file parses as yaml");
    let services = compose
        .get("services")
        .and_then(YamlValue::as_mapping)
        .expect("compose declares services");

    let postgres = service(services, "postgres");
    assert_eq!(scalar(postgres, "image"), "postgres:16");
    assert_sequence_contains(postgres, "ports", "5432:5432");
    assert!(postgres.get("healthcheck").is_some());
    assert_sequence_contains(
        postgres,
        "volumes",
        "opentk-postgres-data:/var/lib/postgresql/data",
    );
    assert_sequence_contains(
        postgres,
        "volumes",
        "./scripts/init-db.sh:/docker-entrypoint-initdb.d/init-db.sh:ro",
    );

    let meilisearch = service(services, "meilisearch");
    assert_eq!(scalar(meilisearch, "image"), "getmeili/meilisearch:latest");
    assert_sequence_contains(meilisearch, "ports", "7700:7700");
    assert!(meilisearch.get("healthcheck").is_some());
    assert_sequence_contains(
        meilisearch,
        "volumes",
        "opentk-meilisearch-data:/meili_data",
    );

    let api = service(services, "api");
    assert_eq!(
        scalar(path_mapping(api, "build"), "dockerfile"),
        "docker/Dockerfile.api"
    );
    assert_sequence_contains(api, "ports", "3000:3000");
    assert_sequence_contains(
        api,
        "volumes",
        "./config/opentk.compose.toml:/etc/opentk/config.toml:ro",
    );
    assert_eq!(
        string_sequence(path_mapping(api, "healthcheck"), "test"),
        [
            "CMD-SHELL",
            "curl -f http://localhost:3000/health || exit 1"
        ]
    );
    assert_eq!(
        depends_condition(api, "postgres"),
        Some("service_healthy"),
        "api must wait for healthy postgres"
    );
    assert!(
        depends_condition(api, "meilisearch").is_none(),
        "api must not hard-block startup on meilisearch health"
    );

    let sync = service(services, "sync");
    assert_eq!(
        scalar(path_mapping(sync, "build"), "dockerfile"),
        "docker/Dockerfile.sync"
    );
    assert_sequence_contains(
        sync,
        "volumes",
        "./config/opentk.compose.toml:/etc/opentk/config.toml:ro",
    );
    assert_eq!(
        depends_condition(sync, "postgres"),
        Some("service_healthy"),
        "sync must wait for healthy postgres"
    );
    assert_eq!(
        string_sequence(sync, "command"),
        ["--config", "/etc/opentk/config.toml", "poll"]
    );
    assert_eq!(
        string_sequence(path_mapping(sync, "healthcheck"), "test"),
        [
            "CMD",
            "/bin/opentk-sync",
            "--config",
            "/etc/opentk/config.toml",
            "--health-check"
        ]
    );
}

#[test]
fn dockerfiles_declare_runtime_healthchecks() {
    let api = fs::read_to_string(workspace_path("docker/Dockerfile.api"))
        .expect("API Dockerfile should exist");
    let sync = fs::read_to_string(workspace_path("docker/Dockerfile.sync"))
        .expect("sync Dockerfile should exist");
    let scratch = fs::read_to_string(workspace_path("docker/Dockerfile.scratch"))
        .expect("scratch Dockerfile should exist");

    assert!(api.contains("HEALTHCHECK CMD curl -f http://localhost:3000/health || exit 1"));
    assert!(sync.contains(
        "HEALTHCHECK CMD /bin/opentk-sync --config /etc/opentk/config.toml --health-check || exit 1"
    ));
    assert!(scratch.contains("HEALTHCHECK NONE"));
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

    let compose_config = fs::read_to_string(workspace_path("config/opentk.compose.toml"))
        .expect("compose config should be readable");
    let compose_config: toml::Value =
        toml::from_str(&compose_config).expect("compose config parses as toml");
    let categories = compose_config
        .get("sync")
        .and_then(|sync| sync.get("categories"))
        .and_then(toml::Value::as_array)
        .expect("compose config should declare sync.categories explicitly");
    let categories: Vec<&str> = categories
        .iter()
        .map(|category| {
            category
                .as_str()
                .expect("sync.categories entries should be strings")
        })
        .collect();
    for category in ["Document", "Zaak", "Activiteit", "Stemming"] {
        assert!(
            categories.contains(&category),
            "compose sync categories should include {category}"
        );
    }
}

#[test]
fn docker_compose_docs_explain_polling_and_cdc_boundaries() {
    let docs = fs::read_to_string(workspace_path("docs/docker-compose.md"))
        .expect("docker compose documentation should exist");

    for required in [
        "docker compose up --build",
        "opentk-sync poll",
        "upstream SyncFeed-to-PostgreSQL",
        "PostgreSQL-to-Meilisearch CDC",
        "curl http://localhost:3000/health",
        "degraded",
        "config/opentk.compose.toml",
    ] {
        assert!(docs.contains(required), "docs should mention {required}");
    }

    let readme = fs::read_to_string(workspace_path("README.md")).expect("README should exist");
    assert!(
        readme.contains("docs/docker-compose.md"),
        "README should link the Docker Compose documentation"
    );
}

#[test]
fn operations_docs_explain_healthchecks_and_shutdown() {
    let docs = fs::read_to_string(workspace_path("docs/operations.md"))
        .expect("operations documentation should exist");

    for required in [
        "/health",
        "degraded",
        "search_sync",
        "opentk-sync --health-check",
        "Docker healthchecks",
        "SIGTERM",
        "graceful shutdown",
    ] {
        assert!(
            docs.contains(required),
            "operations docs should mention {required}"
        );
    }
}

#[test]
fn scratch_docker_build_contract_uses_prebuilt_multi_arch_artifacts() {
    let scratch = fs::read_to_string(workspace_path("docker/Dockerfile.scratch"))
        .expect("scratch Dockerfile should exist");
    let artifacts = fs::read_to_string(workspace_path("docker/Dockerfile.scratch-artifacts"))
        .expect("scratch artifact Dockerfile should exist");
    let script = fs::read_to_string(workspace_path("scripts/docker-buildx-scratch.sh"))
        .expect("scratch buildx script should exist");

    assert_scratch_dockerfile_contract(&scratch);
    assert_scratch_artifact_dockerfile_contract(&artifacts);
    assert_scratch_build_script_contract(&script);
    assert_scratch_docs_contract();
}

#[test]
fn github_docker_workflow_declares_trigger_contract() {
    let workflow = docker_workflow();
    let on = path_mapping(&workflow, "on");
    let push = path_mapping(on, "push");

    assert_sequence_contains(push, "branches", "main");
    assert_sequence_contains(push, "tags", "v*");
    assert!(
        on.contains_key(YamlValue::String("workflow_dispatch".to_owned())),
        "Docker workflow should support manual dispatch"
    );
}

#[test]
fn github_docker_workflow_splits_build_from_publish_auth() {
    let workflow = docker_workflow();
    let jobs = path_mapping(&workflow, "jobs");
    let build = path_mapping(jobs, "build");
    let publish = path_mapping(jobs, "publish");

    assert_eq!(scalar(build, "runs-on"), "ubuntu-24.04");
    assert_eq!(scalar(publish, "runs-on"), "ubuntu-24.04");
    assert_eq!(scalar(publish, "needs"), "build");
    assert!(
        !job_contains(build, "ghcr.io") && !job_contains(build, "docker/login-action"),
        "build job must not authenticate to GHCR"
    );
    assert!(
        job_contains(publish, "ghcr.io") && job_contains(publish, "docker/login-action"),
        "publish job must authenticate to GHCR"
    );
}

#[test]
fn github_docker_workflow_builds_scratch_images_from_repository_dockerfiles() {
    let workflow = docker_workflow();
    let jobs = path_mapping(&workflow, "jobs");
    let build = path_mapping(jobs, "build");

    for required in [
        "docker/setup-buildx-action",
        "docker/Dockerfile.scratch-artifacts",
        "docker/Dockerfile.scratch",
        "opentk-sync",
        "opentk-api",
        "linux/amd64,linux/arm64",
        "--build-arg \"BINARY=${binary}\"",
        "--build-arg \"ARTIFACT_IMAGE=${artifact_image}\"",
        "type=oci,dest=/tmp/${binary}.oci.tar",
        "actions/upload-artifact",
    ] {
        assert!(
            job_contains(build, required),
            "build job should contain {required}"
        );
    }
}

#[test]
fn github_docker_workflow_uses_native_cross_compilation_without_qemu() {
    let workflow = fs::read_to_string(workspace_path(".github/workflows/docker.yml"))
        .expect("Docker GitHub workflow should exist");
    let workflow: YamlValue =
        serde_yaml::from_str(&workflow).expect("Docker GitHub workflow parses as yaml");
    let workflow_text = serde_yaml::to_string(&workflow).expect("workflow serializes");
    let workflow = workflow
        .as_mapping()
        .expect("Docker GitHub workflow should be a mapping");
    let build = path_mapping(path_mapping(workflow, "jobs"), "build");

    for forbidden in ["setup-qemu", "docker/setup-qemu-action", "qemu", "emulat"] {
        assert!(
            !workflow_text.to_lowercase().contains(forbidden),
            "Docker workflow must not contain {forbidden}"
        );
    }
    assert!(
        job_contains(build, "--platform linux/amd64")
            && job_contains(build, "docker/Dockerfile.scratch-artifacts"),
        "artifact build should run once from the native amd64 runner and cross-compile both targets"
    );
}

#[test]
fn github_docker_workflow_caches_cargo_targets_layers_and_final_assembly() {
    let artifacts = fs::read_to_string(workspace_path("docker/Dockerfile.scratch-artifacts"))
        .expect("scratch artifact Dockerfile should exist");
    let workflow = docker_workflow();
    let build = path_mapping(path_mapping(&workflow, "jobs"), "build");

    for required in [
        "--cache-from \"type=gha,scope=opentk-scratch-artifacts\"",
        "--cache-to \"type=gha,scope=opentk-scratch-artifacts,mode=max\"",
        "--cache-from \"type=gha,scope=opentk-scratch-${binary}\"",
        "--cache-to \"type=gha,scope=opentk-scratch-${binary},mode=max\"",
        "opentk-scratch-target",
    ] {
        assert!(
            job_contains(build, required) || artifacts.contains(required),
            "workflow cache contract should contain {required}"
        );
    }
    for required in [
        "--mount=type=cache,target=/usr/local/cargo/registry",
        "--mount=type=cache,target=/usr/local/cargo/git",
        "--mount=type=cache,id=opentk-scratch-target,target=/workspace/target",
    ] {
        assert!(
            artifacts.contains(required),
            "artifact Dockerfile cache contract should contain {required}"
        );
    }
}

#[test]
fn github_docker_workflow_publishes_oci_artifacts_with_main_sha_and_release_tags() {
    let workflow = docker_workflow();
    let publish = path_mapping(path_mapping(&workflow, "jobs"), "publish");

    for required in [
        "actions/download-artifact",
        "scratch-image-oci-archives",
        "apt-get install -y --no-install-recommends skopeo",
        "oci-archive:${binary}.oci.tar",
        "docker://ghcr.io/${owner}/${binary}:${tag}",
        "latest",
        "sha-${short_sha}",
        "${GITHUB_REF_NAME}",
        "refs/heads/main",
        "refs/tags/v",
    ] {
        assert!(
            job_contains(publish, required),
            "publish job should contain {required}"
        );
    }
    assert!(
        !job_contains(publish, "docker buildx build"),
        "publish job must not rebuild images"
    );
}

fn assert_scratch_dockerfile_contract(scratch: &str) {
    for required in [
        "ARG BINARY",
        "ARG ARTIFACT_IMAGE=opentk-scratch-artifacts:local",
        "ARG TARGETARCH",
        "FROM --platform=${BUILDPLATFORM} ${ARTIFACT_IMAGE} AS artifacts",
        "FROM scratch",
        "case \"${TARGETARCH}\" in",
        "amd64) rust_target=\"x86_64-unknown-linux-musl\"",
        "arm64) rust_target=\"aarch64-unknown-linux-musl\"",
        "cp \"/artifacts/${rust_target}/${BINARY}\" /runtime/bin/opentk",
        "COPY --from=artifact-selector /runtime/ /",
        "USER 1000:1000",
        "HEALTHCHECK NONE",
        "ENTRYPOINT [\"/bin/opentk\"]",
    ] {
        assert!(
            scratch.contains(required),
            "scratch Dockerfile should contain {required}"
        );
    }
    for forbidden in [
        "cargo build",
        "apt-get",
        "apk add",
        "/bin/sh",
        "ENTRYPOINT [\"/bin/${BINARY}\"]",
    ] {
        assert!(
            !scratch.contains(forbidden),
            "scratch Dockerfile should not contain {forbidden}"
        );
    }
}

fn assert_scratch_artifact_dockerfile_contract(artifacts: &str) {
    for required in [
        "FROM --platform=${BUILDPLATFORM} rust:1-bookworm AS builder",
        "--mount=type=cache,target=/usr/local/cargo/registry",
        "--mount=type=cache,target=/usr/local/cargo/git",
        "--mount=type=cache,id=opentk-scratch-target,target=/workspace/target",
        "rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl",
        "cargo build --release --target x86_64-unknown-linux-musl -p opentk-db -p opentk-api --bin opentk-sync --bin opentk-api",
        "cargo build --release --target aarch64-unknown-linux-musl -p opentk-db -p opentk-api --bin opentk-sync --bin opentk-api",
        "/artifacts/x86_64-unknown-linux-musl/opentk-sync",
        "/artifacts/x86_64-unknown-linux-musl/opentk-api",
        "/artifacts/aarch64-unknown-linux-musl/opentk-sync",
        "/artifacts/aarch64-unknown-linux-musl/opentk-api",
        "opentk:x:1000:1000:opentk:/nonexistent:/sbin/nologin",
    ] {
        assert!(
            artifacts.contains(required),
            "artifact Dockerfile should contain {required}"
        );
    }
    for forbidden in [
        "target-deps",
        "opentk-scratch-deps-target",
        "opentk-scratch-final-target",
        "dependency_cache_placeholder",
    ] {
        assert!(
            !artifacts.contains(forbidden),
            "artifact Dockerfile should avoid duplicate target cache {forbidden}"
        );
    }
}

fn assert_scratch_build_script_contract(script: &str) {
    for required in [
        "set -euo pipefail",
        "docker buildx create",
        "docker buildx build",
        "-f \"${repo_root}/docker/Dockerfile.scratch-artifacts\"",
        "-f \"${repo_root}/docker/Dockerfile.scratch\"",
        "platforms=\"linux/amd64,linux/arm64\"",
        "--platform \"${platforms}\"",
        "--metadata-file \"${metadata_file}\"",
        "--build-arg \"BINARY=${binary}\"",
        "build_final_image opentk-sync",
        "build_final_image opentk-api",
        "docker run --rm",
        "--help",
        "buildx.build.provenance/${platform}",
        "linux/amd64",
        "linux/arm64",
        "50000000",
        "QEMU",
    ] {
        assert!(
            script.contains(required),
            "scratch buildx script should contain {required}"
        );
    }
}

fn assert_scratch_docs_contract() {
    let docs = fs::read_to_string(workspace_path("docs/docker-scratch.md"))
        .expect("scratch Docker docs should exist");
    for required in [
        "scripts/docker-buildx-scratch.sh",
        "docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=opentk-sync",
        "docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=opentk-api",
        "static scratch runtime images",
        "native Rust cross-compilation",
        "not QEMU",
    ] {
        assert!(docs.contains(required), "scratch docs should mention {required}");
    }

    let readme = fs::read_to_string(workspace_path("README.md")).expect("README should exist");
    assert!(
        readme.contains("docs/docker-scratch.md"),
        "README should link the scratch Docker documentation"
    );
}

#[test]
fn postgres_init_script_runs_migrations_without_ignoring_errors() {
    let script = fs::read_to_string(workspace_path("scripts/init-db.sh"))
        .expect("postgres init script should exist");

    for required in [
        "set -euo pipefail",
        "sqlx migrate run --source /workspace/migrations",
        "psql",
        "--set ON_ERROR_STOP=1",
        "/workspace/migrations/*.up.sql",
    ] {
        assert!(
            script.contains(required),
            "init script should contain {required}"
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

fn docker_workflow() -> serde_yaml::Mapping {
    let workflow = fs::read_to_string(workspace_path(".github/workflows/docker.yml"))
        .expect("Docker GitHub workflow should exist");
    let workflow: YamlValue =
        serde_yaml::from_str(&workflow).expect("Docker GitHub workflow parses as yaml");
    workflow
        .as_mapping()
        .expect("Docker GitHub workflow should be a mapping")
        .clone()
}

fn service<'a>(services: &'a serde_yaml::Mapping, name: &'static str) -> &'a serde_yaml::Mapping {
    services
        .get(YamlValue::String(name.to_owned()))
        .and_then(YamlValue::as_mapping)
        .unwrap_or_else(|| panic!("service {name} should be declared"))
}

fn scalar<'a>(mapping: &'a serde_yaml::Mapping, key: &'static str) -> &'a str {
    mapping
        .get(YamlValue::String(key.to_owned()))
        .and_then(YamlValue::as_str)
        .unwrap_or_else(|| panic!("{key} should be a string"))
}

fn path_mapping<'a>(
    mapping: &'a serde_yaml::Mapping,
    key: &'static str,
) -> &'a serde_yaml::Mapping {
    mapping
        .get(YamlValue::String(key.to_owned()))
        .and_then(YamlValue::as_mapping)
        .unwrap_or_else(|| panic!("{key} should be a mapping"))
}

fn string_sequence<'a>(mapping: &'a serde_yaml::Mapping, key: &'static str) -> Vec<&'a str> {
    mapping
        .get(YamlValue::String(key.to_owned()))
        .and_then(YamlValue::as_sequence)
        .unwrap_or_else(|| panic!("{key} should be a sequence"))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{key} entries should be strings"))
        })
        .collect()
}

fn assert_sequence_contains(mapping: &serde_yaml::Mapping, key: &'static str, expected: &str) {
    let entries = string_sequence(mapping, key);
    assert!(
        entries.contains(&expected),
        "{key} should contain {expected:?}, got {entries:?}"
    );
}

fn depends_condition<'a>(
    mapping: &'a serde_yaml::Mapping,
    service_name: &'static str,
) -> Option<&'a str> {
    mapping
        .get(YamlValue::String("depends_on".to_owned()))
        .and_then(YamlValue::as_mapping)
        .and_then(|depends_on| depends_on.get(YamlValue::String(service_name.to_owned())))
        .and_then(YamlValue::as_mapping)
        .and_then(|dependency| dependency.get(YamlValue::String("condition".to_owned())))
        .and_then(YamlValue::as_str)
}

fn job_contains(job: &serde_yaml::Mapping, needle: &str) -> bool {
    let job = YamlValue::Mapping(job.clone());
    serde_yaml::to_string(&job)
        .expect("job serializes")
        .contains(needle)
}
