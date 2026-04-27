#[test]
fn core_crate_exposes_workspace_name() {
    assert_eq!(opentk_core::workspace_name(), "opentk");
}

#[test]
fn documented_opentk_sync_cargo_command_links_and_prints_help() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("opentk-core crate lives below workspace root");
    let target_dir = workspace_root
        .join("target")
        .join("workspace-smoke")
        .join(format!(
            "opentk-sync-help-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos()
        ));

    let output = std::process::Command::new("cargo")
        .args([
            "run",
            "-p",
            "opentk-db",
            "--bin",
            "opentk-sync",
            "--",
            "--help",
        ])
        .current_dir(workspace_root)
        .env("CARGO_TARGET_DIR", &target_dir)
        .output()
        .expect("cargo command starts");

    assert!(
        output.status.success(),
        "cargo run failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Run or inspect durable Tweede Kamer SyncFeed ingestion"),
        "unexpected help output:\n{stdout}"
    );
}
