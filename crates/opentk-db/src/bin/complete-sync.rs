use std::{path::PathBuf, time::Duration};

use clap::{Parser, Subcommand};
use opentk_config::{Config, ConfigLoader};
use opentk_db::{
    connect,
    sync_state::PostgresSyncStore,
    sync_verification::{verify_sync_database, SyncVerificationConfig},
    DatabaseConfig,
};
use opentk_sync::{
    runner::{CompleteSyncConfig, CompleteSyncRunner, SyncRunMode, SyncStore},
    syncfeed::{SyncFeedClient, SyncFeedClientConfig, SyncFeedContentMode},
};

#[derive(Debug, Parser)]
#[command(name = "complete-sync")]
#[command(about = "Run or inspect durable Tweede Kamer SyncFeed ingestion")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run,
    Poll,
    Status,
    Verify(VerifyArgs),
}

#[derive(Clone, Debug, Parser)]
struct VerifyArgs {
    #[arg(long, default_value_t = 0)]
    required_relation_samples: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let loaded = match cli.config {
        Some(path) => ConfigLoader::with_path(path).load()?,
        None => ConfigLoader::new().load()?,
    };
    match cli.command {
        Command::Run => {
            run_once(&loaded.config).await?;
        }
        Command::Poll => {
            let runner = build_runner(
                &loaded.config,
                SyncRunMode::Continuous,
                Duration::from_secs(loaded.config.sync.poll_interval_secs),
            )
            .await?;
            runner.run_forever().await?;
        }
        Command::Status => {
            print_status(&loaded.config).await?;
        }
        Command::Verify(args) => {
            print_verification(&loaded.config, args).await?;
        }
    }
    Ok(())
}

async fn run_once(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let runner = build_runner(config, SyncRunMode::UntilCaughtUp, Duration::from_secs(1)).await?;
    let report = runner.run_once().await?;
    for category in report.categories {
        println!(
            "{}\tcaught_up={}\tlatest_skiptoken={}\tpages_written={}\tentities_seen={}",
            category.category,
            category.caught_up,
            display_optional_i64(category.latest_skiptoken),
            category.pages_written,
            category.entities_seen
        );
    }
    Ok(())
}

async fn print_status(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let pool = connect(&database_config(config)).await?;
    let store = PostgresSyncStore::new(pool);
    for status in store.status(&config.sync.categories).await? {
        let last_error = status.last_error.map_or_else(
            || "none".to_owned(),
            |error| {
                format!(
                    "{} skiptoken={} entity={} {}",
                    error.phase.as_str(),
                    display_optional_i64(error.skiptoken),
                    error
                        .entity_id
                        .map_or_else(|| "null".to_owned(), |entity_id| entity_id.to_string()),
                    error.message
                )
            },
        );
        println!(
            "{}\tstate={}\tlatest_skiptoken={}\tlag_seconds={}\tlast_fetch_at={}\tlast_error={}",
            status.category,
            status.state.as_str(),
            display_optional_i64(status.latest_skiptoken),
            status
                .lag
                .map_or_else(|| "null".to_owned(), |lag| lag.as_secs().to_string()),
            status.last_fetch_at.map_or_else(
                || "null".to_owned(),
                |last_fetch_at| last_fetch_at.to_rfc3339()
            ),
            last_error
        );
    }
    Ok(())
}

async fn print_verification(
    config: &Config,
    args: VerifyArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = connect(&database_config(config)).await?;
    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: config.sync.categories.clone(),
            required_relation_samples: args.required_relation_samples,
        },
    )
    .await?;
    println!(
        "categories={}\trelations={}\tdirect_queries={}\ttable_bytes={}\tindex_bytes={}\tlink_metadata_bytes={}\textracted_text_bytes={}\tstored_html_bytes={}\tconstraint_count={}\tbinary_asset_metadata_rows={}\tdocument_content_rows={}",
        report.categories.len(),
        report.relation_tables.len(),
        report.direct_queries.len(),
        report.storage.table_bytes,
        report.storage.index_bytes,
        report.storage.link_metadata_bytes,
        report.storage.extracted_text_bytes,
        report.storage.stored_html_bytes,
        report.storage.constraint_count,
        report.storage.binary_asset_metadata_rows,
        report.storage.document_content_rows
    );
    for category in report.categories {
        println!(
            "category={}\ttable={}\tcurrent_rows={}\tregistry_rows={}\tstate={}\tlatest_skiptoken={}",
            category.category,
            category.table_name,
            category.current_rows,
            category.registry_rows,
            category.state.as_str(),
            display_optional_i64(category.latest_skiptoken)
        );
    }
    for relation in report.relation_tables {
        println!(
            "relation={}.{}\ttable={}\trows={}\tqueryable_from_source={}\tqueryable_from_target={}",
            relation.source_category,
            relation.relation_name,
            relation.table_name,
            relation.rows,
            relation.queryable_from_source,
            relation.queryable_from_target
        );
    }
    for query in report.direct_queries {
        println!("direct_query={}\trows={}", query.name, query.rows);
    }
    Ok(())
}

async fn build_runner(
    config: &Config,
    mode: SyncRunMode,
    poll_interval: Duration,
) -> Result<CompleteSyncRunner<PostgresSyncStore>, Box<dyn std::error::Error>> {
    let pool = connect(&database_config(config)).await?;
    let client = SyncFeedClient::new(SyncFeedClientConfig {
        base_url: config.sync.base_url.clone(),
        content_mode: SyncFeedContentMode::Internal,
        request_timeout: Duration::from_secs(config.sync.request_timeout_secs),
        connect_timeout: Duration::from_secs(config.sync.connect_timeout_secs),
        max_retries: config.sync.max_retries,
        initial_retry_delay: Duration::from_millis(500),
        max_retry_delay: Duration::from_secs(30),
        max_concurrent_requests: config.sync.max_concurrent_requests,
    })?;
    Ok(CompleteSyncRunner {
        client,
        store: PostgresSyncStore::new(pool),
        config: CompleteSyncConfig {
            categories: config.sync.categories.clone(),
            mode,
            poll_interval,
        },
    })
}

fn database_config(config: &Config) -> DatabaseConfig {
    DatabaseConfig {
        url: config.database.url.clone(),
        max_connections: config.database.max_connections,
    }
}

fn display_optional_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn run_command_accepts_config_path_only() {
        let cli = Cli::try_parse_from([
            "complete-sync",
            "--config",
            "/etc/opentk/config.toml",
            "run",
        ])
        .expect("run command parses");

        let Command::Run = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/etc/opentk/config.toml"))
        );
    }

    #[test]
    fn old_application_setting_flags_are_rejected() {
        for args in [
            [
                "complete-sync",
                "run",
                "--database-url",
                "postgres://example.test/db",
            ],
            [
                "complete-sync",
                "run",
                "--base-url",
                "https://sync.example.test",
            ],
            ["complete-sync", "run", "--category", "Document"],
            ["complete-sync", "poll", "--poll-interval-seconds", "5"],
        ] {
            assert!(Cli::try_parse_from(args).is_err(), "{args:?} must fail");
        }
    }

    #[test]
    fn status_command_has_no_application_setting_args() {
        let cli = Cli::try_parse_from(["complete-sync", "status"]).expect("status command parses");
        let Command::Status = cli.command else {
            panic!("expected status command");
        };
    }

    #[test]
    fn verify_command_accepts_only_relation_sample_requirement() {
        let cli = Cli::try_parse_from([
            "complete-sync",
            "verify",
            "--required-relation-samples",
            "1",
        ])
        .expect("verify command parses");

        let Command::Verify(args) = cli.command else {
            panic!("expected verify command");
        };
        assert_eq!(args.required_relation_samples, 1);
    }
}
