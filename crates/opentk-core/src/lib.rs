//! Source-neutral domain and shared value types for `OpenTK`.

pub mod official_schema;

/// Returns the canonical workspace name used by setup smoke tests and tooling.
#[must_use]
pub const fn workspace_name() -> &'static str {
    "opentk"
}
