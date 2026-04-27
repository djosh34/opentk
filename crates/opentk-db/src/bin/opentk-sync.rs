use std::{fmt::Write as _, path::PathBuf, time::Duration};

use clap::{Parser, Subcommand};
use opentk_config::{Config, ConfigLoader};
use opentk_db::{
    connect,
    startup_validation::validate_sync_dependencies,
    sync_state::PostgresSyncStore,
    sync_verification::{verify_sync_database, SyncVerificationConfig, SyncVerificationReport},
    DatabaseConfig,
};
use opentk_sync::{
    runner::{CompleteSyncConfig, CompleteSyncRunner, SyncRunMode, SyncStore},
    syncfeed::{SyncFeedClient, SyncFeedClientConfig, SyncFeedContentMode},
};

#[derive(Debug, Parser)]
#[command(name = "opentk-sync")]
#[command(about = "Run or inspect durable Tweede Kamer SyncFeed ingestion")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    validate_config: bool,
    #[arg(long, global = true)]
    health_check: bool,
    #[command(subcommand)]
    command: Option<Command>,
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
    let config = loaded.config;
    tracing_subscriber::fmt::init();
    tracing::info!(config = ?config.redacted(), "loaded effective config");
    if cli.validate_config {
        let report = validate_sync_dependencies(&config).await?;
        print!("{report}");
        return Ok(());
    }
    if cli.health_check {
        check_database_health(&config).await?;
        return Ok(());
    }

    let command = cli
        .command
        .ok_or("command is required unless --validate-config or --health-check is set")?;
    match command {
        Command::Run => {
            run_once(&config).await?;
        }
        Command::Poll => {
            let runner = build_runner(
                &config,
                SyncRunMode::Continuous,
                Duration::from_secs(config.sync.poll_interval_secs),
            )
            .await?;
            runner.run_until_shutdown(shutdown_signal()).await?;
        }
        Command::Status => {
            print_status(&config).await?;
        }
        Command::Verify(args) => {
            print_verification(&config, args).await?;
        }
    }
    Ok(())
}

async fn check_database_health(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let pool = connect(&database_config(config)).await?;
    sqlx::query("SELECT 1").execute(&pool).await?;
    pool.close().await;
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
    print!("{}", format_verification_report(&report));
    Ok(())
}

fn format_verification_report(report: &SyncVerificationReport) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "categories={}\trelations={}\tdirect_queries={}\ttable_bytes={}\tindex_bytes={}\tlink_metadata_bytes={}\textracted_text_bytes={}\textracted_html_bytes={}\tconstraint_count={}\tbinary_asset_metadata_rows={}\tdocument_content_rows={}\tingest_error_count={}",
        report.categories.len(),
        report.relation_tables.len(),
        report.direct_queries.len(),
        report.storage.table_bytes,
        report.storage.index_bytes,
        report.storage.link_metadata_bytes,
        report.storage.extracted_text_bytes,
        report.storage.extracted_html_bytes,
        report.storage.constraint_count,
        report.storage.binary_asset_metadata_rows,
        report.storage.document_content_rows,
        report.ingest_error_count
    )
    .expect("writing verification report line to String cannot fail");
    for error in &report.recent_ingest_errors {
        writeln!(
            output,
            "ingest_error_id={}\tphase={}\tsource_category={}\tsource_id={}\tlatest_skiptoken={}\tmessage={}\tcreated_at={}",
            error.id,
            error.phase,
            error.source_category,
            error
                .source_id
                .map_or_else(|| "null".to_owned(), |source_id| source_id.to_string()),
            display_optional_i64(error.latest_skiptoken),
            error.message,
            error.created_at.to_rfc3339()
        )
        .expect("writing ingest-error verification line to String cannot fail");
    }
    for category in &report.categories {
        writeln!(
            output,
            "category={}\ttable={}\tcurrent_rows={}\tregistry_rows={}\tstate={}\tlatest_skiptoken={}",
            category.category,
            category.table_name,
            category.current_rows,
            category.registry_rows,
            category.state.as_str(),
            display_optional_i64(category.latest_skiptoken)
        )
        .expect("writing category verification line to String cannot fail");
    }
    for relation in &report.relation_tables {
        writeln!(
            output,
            "relation={}.{}\ttable={}\trows={}\tqueryable_from_source={}\tqueryable_from_target={}",
            relation.source_category,
            relation.relation_name,
            relation.table_name,
            relation.rows,
            relation.queryable_from_source,
            relation.queryable_from_target
        )
        .expect("writing relation verification line to String cannot fail");
    }
    for query in &report.direct_queries {
        writeln!(output, "direct_query={}\trows={}", query.name, query.rows)
            .expect("writing direct-query verification line to String cannot fail");
    }
    output
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

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl-C handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{format_verification_report, Cli, Command};
    use chrono::{DateTime, Utc};
    use clap::Parser;
    use opentk_db::sync_verification::{
        DirectQueryVerification, IngestErrorVerification, StorageVerification,
        SyncVerificationReport,
    };
    use uuid::Uuid;

    #[test]
    fn run_command_accepts_config_path_only() {
        let cli =
            Cli::try_parse_from(["opentk-sync", "--config", "/etc/opentk/config.toml", "run"])
                .expect("run command parses");

        let Some(Command::Run) = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/etc/opentk/config.toml"))
        );
    }

    #[test]
    fn sync_cli_accepts_validation_without_subcommand() {
        let cli = Cli::try_parse_from(["opentk-sync", "--validate-config"])
            .expect("validation mode parses");
        assert!(cli.validate_config);
        assert!(cli.command.is_none());
    }

    #[test]
    fn sync_cli_accepts_health_check_without_subcommand() {
        let cli = Cli::try_parse_from(["opentk-sync", "--health-check"])
            .expect("health check mode parses");
        assert!(cli.health_check);
        assert!(cli.command.is_none());
    }

    #[test]
    fn old_application_setting_flags_are_rejected() {
        for args in [
            [
                "opentk-sync",
                "run",
                "--database-url",
                "postgres://example.test/db",
            ],
            [
                "opentk-sync",
                "run",
                "--base-url",
                "https://sync.example.test",
            ],
            ["opentk-sync", "run", "--category", "Document"],
            ["opentk-sync", "poll", "--poll-interval-seconds", "5"],
        ] {
            assert!(Cli::try_parse_from(args).is_err(), "{args:?} must fail");
        }
    }

    #[test]
    fn status_command_has_no_application_setting_args() {
        let cli = Cli::try_parse_from(["opentk-sync", "status"]).expect("status command parses");
        let Some(Command::Status) = cli.command else {
            panic!("expected status command");
        };
    }

    #[test]
    fn verify_command_accepts_only_relation_sample_requirement() {
        let cli =
            Cli::try_parse_from(["opentk-sync", "verify", "--required-relation-samples", "1"])
                .expect("verify command parses");

        let Some(Command::Verify(args)) = cli.command else {
            panic!("expected verify command");
        };
        assert_eq!(args.required_relation_samples, 1);
    }

    #[test]
    fn verify_output_reports_schema_backed_ingest_errors_and_extracted_html_bytes() {
        let created_at: DateTime<Utc> = "2026-04-26T03:04:05Z".parse().expect("valid timestamp");
        let source_id =
            Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-000000000001").expect("valid uuid");
        let report = SyncVerificationReport {
            categories: Vec::new(),
            relation_tables: Vec::new(),
            direct_queries: vec![DirectQueryVerification {
                name: "recent_registry_changes".to_owned(),
                rows: 7,
            }],
            table_snapshots: Vec::new(),
            storage: StorageVerification {
                table_bytes: 100,
                index_bytes: 20,
                link_metadata_bytes: 10,
                extracted_text_bytes: 30,
                extracted_html_bytes: 40,
                constraint_count: 5,
                binary_asset_metadata_rows: 2,
                document_content_rows: 3,
            },
            ingest_error_count: 1,
            recent_ingest_errors: vec![IngestErrorVerification {
                id: 9,
                phase: "parse_entity".to_owned(),
                source_category: "Document".to_owned(),
                source_id: Some(source_id),
                latest_skiptoken: Some(42),
                message: "invalid source payload".to_owned(),
                created_at,
            }],
        };

        let output = format_verification_report(&report);

        assert!(output.contains("extracted_html_bytes=40"));
        assert!(output.contains("ingest_error_count=1"));
        assert!(output.contains("ingest_error_id=9"));
        assert!(output.contains("phase=parse_entity"));
        assert!(output.contains("source_category=Document"));
        assert!(output.contains("source_id=aaaaaaaa-aaaa-4aaa-8aaa-000000000001"));
        assert!(output.contains("latest_skiptoken=42"));
        assert!(output.contains("message=invalid source payload"));
        assert!(output.contains("created_at=2026-04-26T03:04:05+00:00"));
        assert!(!output.contains(&["stored", "_html"].concat()));
        assert!(!output.contains(&["occurred", "_at"].concat()));
    }
}
