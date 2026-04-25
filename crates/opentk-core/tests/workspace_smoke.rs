#[test]
fn core_crate_exposes_workspace_name() {
    assert_eq!(opentk_core::workspace_name(), "opentk");
}
