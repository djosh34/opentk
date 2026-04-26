use std::env::VarError;

use clap::{Parser, Subcommand};
use opentk_db::search_sync::{
    full_reindex, incremental_index, list_failures, SearchSyncConfig, SearchSyncReport,
};
use opentk_search::MeilisearchClient;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(name = "search-sync")]
#[command(about = "Build or update the OpenTK Meilisearch index from PostgreSQL")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    FullReindex(SyncArgs),
    Incremental(SyncArgs),
    Failures(FailureArgs),
    RetryFailures(SyncArgs),
}

#[derive(Clone, Debug, Parser)]
struct SyncArgs {
    #[arg(long, env = "OPENTK_DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long, env = "OPENTK_MEILISEARCH_URL")]
    meilisearch_url: String,
    #[arg(long, env = "OPENTK_MEILISEARCH_API_KEY")]
    meilisearch_api_key: Option<String>,
    #[arg(long, default_value = "opentk_entities")]
    index_name: String,
    #[arg(long = "category")]
    categories: Vec<String>,
    #[arg(long, default_value_t = 100)]
    batch_size: i64,
    #[arg(long, default_value_t = 3)]
    retry_limit: i32,
}

#[derive(Clone, Debug, Parser)]
struct FailureArgs {
    #[arg(long, env = "OPENTK_DATABASE_URL")]
    database_url: Option<String>,
}

#[derive(Debug, Error)]
enum DatabaseUrlError {
    #[error("provide --database-url, OPENTK_DATABASE_URL, or DATABASE_URL")]
    Missing,
    #[error("DATABASE_URL must contain valid Unicode")]
    InvalidFallbackUnicode,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::FullReindex(args) => {
            let report = run_sync(args, true).await?;
            print_report(&report);
        }
        Command::Incremental(args) | Command::RetryFailures(args) => {
            let report = run_sync(args, false).await?;
            print_report(&report);
        }
        Command::Failures(args) => {
            let pool = connect(&database_url(args.database_url)?).await?;
            for failure in list_failures(&pool).await? {
                println!(
                    "{}\t{}\t{}\t{}\tattempts={}\tnext_retry_at={}\terror={}",
                    failure.index_name,
                    failure.source_category,
                    failure.source_id,
                    failure.operation,
                    failure.attempt_count,
                    failure.next_retry_at.to_rfc3339(),
                    failure.error
                );
            }
        }
    }
    Ok(())
}

async fn run_sync(
    args: SyncArgs,
    full: bool,
) -> Result<SearchSyncReport, Box<dyn std::error::Error>> {
    let pool = connect(&database_url(args.database_url)?).await?;
    let config = SearchSyncConfig {
        index_name: args.index_name.clone(),
        categories: if args.categories.is_empty() {
            SearchSyncConfig::default().categories
        } else {
            args.categories
        },
        batch_size: args.batch_size,
        retry_limit: args.retry_limit,
    };
    let client = MeilisearchClient::new(
        args.meilisearch_url,
        args.meilisearch_api_key,
        config.index_name.clone(),
    );
    if full {
        Ok(full_reindex(&pool, &client, &config).await?)
    } else {
        Ok(incremental_index(&pool, &client, &config).await?)
    }
}

async fn connect(database_url: &str) -> Result<sqlx::PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

fn print_report(report: &SearchSyncReport) {
    println!(
        "mode={:?}\tindexed={}\tdeleted={}\tfailed={}",
        report.mode, report.indexed, report.deleted, report.failed
    );
    for cursor in &report.latest_cursors {
        println!(
            "{}\t{}\tlatest_skiptoken={}\tstate={}",
            cursor.index_name, cursor.source_category, cursor.latest_skiptoken, cursor.state
        );
    }
}

fn database_url(value: Option<String>) -> Result<String, DatabaseUrlError> {
    match value {
        Some(value) => Ok(value),
        None => match std::env::var("DATABASE_URL") {
            Ok(value) => Ok(value),
            Err(VarError::NotPresent) => Err(DatabaseUrlError::Missing),
            Err(VarError::NotUnicode(_)) => Err(DatabaseUrlError::InvalidFallbackUnicode),
        },
    }
}
