use std::{fmt::Write as _, path::PathBuf, time::Duration};

use clap::{Parser, Subcommand};
use opentk_config::{init_tracing, Config, ConfigLoader};
use opentk_db::{
    connect,
    schema_lifecycle::ensure_schema,
    search_reconciler::{reconcile_once, SearchReconcilerConfig, SearchReconcilerReport},
    startup_validation::{validate_database_config, validate_meilisearch_config},
    DatabaseConfig,
};
use opentk_search::{MeilisearchClient, SearchHealthClient};

#[derive(Debug, Parser)]
#[command(name = "opentk-search-reconciler")]
#[command(about = "Continuously reconcile OpenTK Meilisearch indexes from PostgreSQL")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    health_check: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run,
    Once,
    Status,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let loaded = match cli.config {
        Some(path) => ConfigLoader::with_path(path).load()?,
        None => ConfigLoader::new().load()?,
    };
    let config = loaded.config;
    init_tracing(&config.log)?;
    tracing::info!(config = ?config.redacted(), "loaded effective config");

    if cli.health_check {
        check_health(&config).await?;
        return Ok(());
    }

    match cli.command.unwrap_or(Command::Run) {
        Command::Run => run_loop(&config).await?,
        Command::Once | Command::Status => {
            let report = run_once(&config).await?;
            print!("{}", format_report(&report));
        }
    }
    Ok(())
}

async fn run_loop(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let reconciler_config = reconciler_config(config);
    loop {
        match run_once(config).await {
            Ok(report) => print!("{}", format_report(&report)),
            Err(error) => {
                tracing::error!(
                    error = %error,
                    retry_after_secs = reconciler_config.loop_interval.as_secs(),
                    "search reconciler pass failed; retrying"
                );
            }
        }
        tokio::time::sleep(reconciler_config.loop_interval).await;
    }
}

async fn run_once(config: &Config) -> Result<SearchReconcilerReport, Box<dyn std::error::Error>> {
    let pool = connect(&database_config(config)).await?;
    ensure_schema(&pool).await?;
    Ok(reconcile_once(&pool, &search_client(config), &reconciler_config(config)).await?)
}

async fn check_health(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    validate_database_config(&database_config(config)).await?;
    validate_meilisearch_config(
        config.search.url.clone(),
        config.search.api_key.clone(),
        config.search.index_name.clone(),
    )
    .await?;
    search_client(config).health().await?;
    Ok(())
}

fn database_config(config: &Config) -> DatabaseConfig {
    DatabaseConfig {
        url: config.database.url.clone(),
        max_connections: 1,
    }
}

fn search_client(config: &Config) -> MeilisearchClient {
    MeilisearchClient::new(
        config.search.url.clone(),
        config.search.api_key.clone(),
        config.search.index_name.clone(),
    )
}

fn reconciler_config(config: &Config) -> SearchReconcilerConfig {
    SearchReconcilerConfig {
        index_name: config.search.index_name.clone(),
        categories: config.sync.categories.clone(),
        batch_size: config.search.batch_size,
        max_payload_bytes: config.search.max_payload_bytes,
        loop_interval: Duration::from_secs(config.sync.poll_interval_secs.max(1)),
    }
}

fn format_report(report: &SearchReconcilerReport) -> String {
    let mut output = String::new();
    writeln!(output, "category_count={}", report.categories.len())
        .expect("writing report to String cannot fail");
    for category in &report.categories {
        writeln!(
            output,
            "source_category={}\tpostgres_count={}\tmeilisearch_count={}\thighest_meilisearch_skiptoken={}\tverified_prefix_boundary={}\ttarget_boundary={}\tscratch_row_count={}\tpayload_bytes={}\tinserted_rows={}\tdeleted_rows={}\tcompleted={}\tloop_duration_ms={}",
            category.source_category,
            category.postgres_count,
            category.meilisearch_count,
            display_optional_i64(category.highest_meilisearch_skiptoken),
            category.verified_prefix_boundary,
            display_optional_i64(category.target_boundary),
            category.scratch_row_count,
            category.payload_bytes,
            category.inserted_rows,
            category.deleted_rows,
            category.completed,
            category.loop_duration_ms,
        )
        .expect("writing category report to String cannot fail");
    }
    output
}

fn display_optional_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}
