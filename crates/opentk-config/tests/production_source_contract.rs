use std::{fs, path::Path};

#[test]
fn sync_binary_is_named_opentk_sync_not_complete_sync() {
    let workspace = workspace();

    assert!(
        workspace
            .join("crates/opentk-db/src/bin/opentk-sync.rs")
            .exists(),
        "sync binary source must be named opentk-sync.rs"
    );
    assert!(
        !workspace
            .join("crates/opentk-db/src/bin/complete-sync.rs")
            .exists(),
        "old complete-sync binary source must be removed"
    );
    assert!(
        !workspace
            .join("crates/opentk-db/src/bin/search-sync.rs")
            .exists(),
        "old search-sync binary source must be removed"
    );

    let db_manifest = fs::read_to_string(workspace.join("crates/opentk-db/Cargo.toml"))
        .expect("read opentk-db manifest");
    assert!(
        db_manifest.contains("name = \"opentk-sync\""),
        "opentk-db manifest must expose the opentk-sync binary"
    );
    assert!(
        !db_manifest.contains("complete-sync"),
        "opentk-db manifest must not expose the old complete-sync binary"
    );
    assert!(
        !db_manifest.contains("search-sync"),
        "opentk-db manifest must not expose the old search-sync binary"
    );
}

#[test]
fn production_application_config_does_not_read_individual_setting_env_vars() {
    let workspace = workspace();
    for relative in [
        "Cargo.toml",
        "crates/opentk-config/src/lib.rs",
        "crates/opentk-api/src/bin/opentk-api.rs",
        "crates/opentk-api/src/lib.rs",
        "crates/opentk-db/src/bin/opentk-sync.rs",
    ] {
        let path = workspace.join(relative);
        let source = fs::read_to_string(&path).expect("read production source");
        for forbidden in [
            "OPENTK_",
            "DATABASE_URL",
            "dotenvy",
            "std::env::var",
            "env = ",
            "features = [\"derive\", \"env\"]",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must not contain {forbidden:?}"
            );
        }
    }
}

#[test]
fn production_search_defaults_live_only_in_unified_config() {
    let workspace = workspace();

    for relative in [
        "crates/opentk-api/src/bin/opentk-api.rs",
        "crates/opentk-api/src/lib.rs",
        "crates/opentk-db/src/bin/opentk-sync.rs",
    ] {
        let source = fs::read_to_string(workspace.join(relative)).expect("read production source");
        for forbidden in [
            "DEFAULT_SEARCH_URL",
            "DEFAULT_SEARCH_INDEX",
            "default_search_client",
            "pub fn router(pool",
            "127.0.0.1:7700",
            "OPENTK_MEILISEARCH",
            "OPENTK_SEARCH",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must not contain {forbidden:?}"
            );
        }
    }
}

#[test]
fn runtime_binaries_initialize_plain_fmt_logging_and_log_redacted_config() {
    let workspace = workspace();

    for relative in [
        "crates/opentk-api/src/bin/opentk-api.rs",
        "crates/opentk-db/src/bin/opentk-sync.rs",
    ] {
        let source = fs::read_to_string(workspace.join(relative)).expect("read binary source");
        assert!(
            source.contains("tracing_subscriber::fmt::init();"),
            "{relative} must initialize tracing with tracing_subscriber::fmt::init()"
        );
        assert!(
            source.contains("config.redacted()"),
            "{relative} must log the shared redacted config view"
        );
        assert!(
            !source.contains("EnvFilter"),
            "{relative} must not hand-roll logging filters"
        );
    }
}

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace dir")
}
