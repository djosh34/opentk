use std::{env::VarError, num::NonZeroUsize, time::Duration};

use clap::{Parser, Subcommand};
use opentk_core::official_schema;
use opentk_db::{
    sync_state::PostgresSyncStore,
    sync_verification::{verify_sync_database, SyncVerificationConfig},
};
use opentk_sync::{
    runner::{CompleteSyncConfig, CompleteSyncRunner, SyncRunMode, SyncStore},
    syncfeed::{SyncFeedClient, SyncFeedClientConfig, SyncFeedContentMode},
};
use reqwest::Url;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

const DEFAULT_SYNCFEED_BASE_URL: &str = "https://gegevensmagazijn.tweedekamer.nl";

#[derive(Debug, Parser)]
#[command(name = "complete-sync")]
#[command(about = "Run or inspect durable Tweede Kamer SyncFeed ingestion")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunArgs),
    Poll(PollArgs),
    Status(StatusArgs),
    Verify(VerifyArgs),
}

#[derive(Debug, Error)]
enum DatabaseUrlError {
    #[error("provide --database-url, OPENTK_DATABASE_URL, or DATABASE_URL")]
    Missing,
    #[error("DATABASE_URL must contain valid Unicode")]
    InvalidFallbackUnicode,
}

#[derive(Clone, Debug, Parser)]
struct RunArgs {
    #[arg(long, env = "OPENTK_DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long, default_value = DEFAULT_SYNCFEED_BASE_URL)]
    base_url: Url,
    #[arg(long = "category")]
    categories: Vec<String>,
}

#[derive(Clone, Debug, Parser)]
struct PollArgs {
    #[arg(long, env = "OPENTK_DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long, default_value = DEFAULT_SYNCFEED_BASE_URL)]
    base_url: Url,
    #[arg(long = "category")]
    categories: Vec<String>,
    #[arg(long, default_value_t = 30)]
    poll_interval_seconds: u64,
}

#[derive(Clone, Debug, Parser)]
struct StatusArgs {
    #[arg(long, env = "OPENTK_DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long = "category")]
    categories: Vec<String>,
}

#[derive(Clone, Debug, Parser)]
struct VerifyArgs {
    #[arg(long, env = "OPENTK_DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long = "category")]
    categories: Vec<String>,
    #[arg(long, default_value_t = 0)]
    required_relation_samples: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run(args) => {
            run_once(args).await?;
        }
        Command::Poll(args) => {
            let runner = build_runner(
                database_url(args.database_url)?,
                args.base_url,
                categories_or_all(args.categories),
                SyncRunMode::Continuous,
                Duration::from_secs(args.poll_interval_seconds),
            )
            .await?;
            runner.run_forever().await?;
        }
        Command::Status(args) => {
            print_status(args).await?;
        }
        Command::Verify(args) => {
            print_verification(args).await?;
        }
    }
    Ok(())
}

async fn run_once(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let runner = build_runner(
        database_url(args.database_url)?,
        args.base_url,
        categories_or_all(args.categories),
        SyncRunMode::UntilCaughtUp,
        Duration::from_secs(1),
    )
    .await?;
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

async fn print_status(args: StatusArgs) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url(args.database_url)?)
        .await?;
    let store = PostgresSyncStore::new(pool);
    for status in store.status(&categories_or_all(args.categories)).await? {
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

async fn print_verification(args: VerifyArgs) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url(args.database_url)?)
        .await?;
    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: categories_or_all(args.categories),
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
    database_url: String,
    base_url: Url,
    categories: Vec<String>,
    mode: SyncRunMode,
    poll_interval: Duration,
) -> Result<CompleteSyncRunner<PostgresSyncStore>, Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    let client = SyncFeedClient::new(SyncFeedClientConfig {
        base_url,
        content_mode: SyncFeedContentMode::Internal,
        request_timeout: Duration::from_secs(30),
        connect_timeout: Duration::from_secs(10),
        max_retries: 3,
        initial_retry_delay: Duration::from_millis(500),
        max_retry_delay: Duration::from_secs(30),
        max_concurrent_requests: NonZeroUsize::new(4).expect("non-zero"),
    })?;
    Ok(CompleteSyncRunner {
        client,
        store: PostgresSyncStore::new(pool),
        config: CompleteSyncConfig {
            categories,
            mode,
            poll_interval,
        },
    })
}

fn database_url(argument: Option<String>) -> Result<String, DatabaseUrlError> {
    if let Some(argument) = argument {
        return Ok(argument);
    }
    match std::env::var("DATABASE_URL") {
        Ok(database_url) => Ok(database_url),
        Err(VarError::NotPresent) => Err(DatabaseUrlError::Missing),
        Err(VarError::NotUnicode(_)) => Err(DatabaseUrlError::InvalidFallbackUnicode),
    }
}

fn categories_or_all(categories: Vec<String>) -> Vec<String> {
    if categories.is_empty() {
        official_schema::entity_types()
            .iter()
            .map(|entity| entity.category.to_owned())
            .collect()
    } else {
        categories
    }
}

fn display_optional_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{categories_or_all, database_url, Cli, Command};
    use clap::Parser;
    #[cfg(unix)]
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    #[cfg(unix)]
    use std::sync::Mutex;

    #[cfg(unix)]
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn run_command_accepts_explicit_category_selection() {
        let cli = Cli::try_parse_from([
            "complete-sync",
            "run",
            "--database-url",
            "postgres://postgres@example.test/opentk",
            "--base-url",
            "https://sync.example.test",
            "--category",
            "Document",
            "--category",
            "Zaak",
        ])
        .expect("run command parses");

        let Command::Run(args) = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(args.categories, ["Document", "Zaak"]);
        assert_eq!(args.base_url.as_str(), "https://sync.example.test/");
    }

    #[test]
    fn empty_category_selection_expands_to_official_categories() {
        let categories = categories_or_all(Vec::new());

        assert!(categories.contains(&"Document".to_owned()));
        assert!(categories.contains(&"Zaak".to_owned()));
        assert!(categories.len() > 2);
    }

    #[test]
    fn status_command_accepts_database_and_categories() {
        let cli = Cli::try_parse_from([
            "complete-sync",
            "status",
            "--database-url",
            "postgres://postgres@example.test/opentk",
            "--category",
            "Document",
        ])
        .expect("status command parses");

        let Command::Status(args) = cli.command else {
            panic!("expected status command");
        };
        assert_eq!(args.categories, ["Document"]);
    }

    #[test]
    fn verify_command_accepts_categories_and_relation_sample_requirement() {
        let cli = Cli::try_parse_from([
            "complete-sync",
            "verify",
            "--database-url",
            "postgres://postgres@example.test/opentk",
            "--category",
            "Document",
            "--required-relation-samples",
            "1",
        ])
        .expect("verify command parses");

        let Command::Verify(args) = cli.command else {
            panic!("expected verify command");
        };
        assert_eq!(args.categories, ["Document"]);
        assert_eq!(args.required_relation_samples, 1);
    }

    #[cfg(unix)]
    #[test]
    fn database_url_reports_invalid_unicode_fallback_env() {
        let _guard = ENV_LOCK.lock().expect("env lock is not poisoned");
        let original = std::env::var_os("DATABASE_URL");
        std::env::set_var("DATABASE_URL", OsString::from_vec(vec![0x66, 0x80, 0x6f]));

        let error = database_url(None).expect_err("invalid unicode is an error");

        match original {
            Some(value) => std::env::set_var("DATABASE_URL", value),
            None => std::env::remove_var("DATABASE_URL"),
        }
        let message = error.to_string();
        assert!(message.contains("DATABASE_URL"));
        assert!(message.contains("valid Unicode"));
        assert!(!message.contains("provide --database-url"));
    }
}
