use std::{fs, path::Path};

#[test]
fn production_application_config_does_not_read_individual_setting_env_vars() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace dir");
    for relative in [
        "Cargo.toml",
        "crates/opentk-config/src/lib.rs",
        "crates/opentk-api/src/bin/opentk-api.rs",
        "crates/opentk-api/src/lib.rs",
        "crates/opentk-db/src/bin/complete-sync.rs",
        "crates/opentk-db/src/bin/search-sync.rs",
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
