use std::{num::NonZeroUsize, time::Duration};

use clap::{Parser, Subcommand};
use opentk_core::official_schema;
use opentk_db::sync_state::PostgresSyncStore;
use opentk_sync::{
    runner::{CompleteSyncConfig, CompleteSyncRunner, SyncRunMode, SyncStore},
    syncfeed::{SyncFeedClient, SyncFeedClientConfig, SyncFeedContentMode},
};
use reqwest::Url;
use sqlx::postgres::PgPoolOptions;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run(args) => {
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
                    category
                        .latest_skiptoken
                        .map_or_else(|| "null".to_owned(), |skiptoken| skiptoken.to_string()),
                    category.pages_written,
                    category.entities_seen
                );
            }
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
                            error.skiptoken.map_or_else(
                                || "null".to_owned(),
                                |skiptoken| skiptoken.to_string()
                            ),
                            error.entity_id.map_or_else(
                                || "null".to_owned(),
                                |entity_id| entity_id.to_string()
                            ),
                            error.message
                        )
                    },
                );
                println!(
                    "{}\tstate={}\tlatest_skiptoken={}\tlag_seconds={}\tlast_fetch_at={}\tlast_error={}",
                    status.category,
                    status.state.as_str(),
                    status
                        .latest_skiptoken
                        .map_or_else(|| "null".to_owned(), |skiptoken| skiptoken.to_string()),
                    status
                        .lag
                        .map_or_else(|| "null".to_owned(), |lag| lag.as_secs().to_string()),
                    status
                        .last_fetch_at
                        .map_or_else(|| "null".to_owned(), |last_fetch_at| last_fetch_at.to_rfc3339()),
                    last_error
                );
            }
        }
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

fn database_url(argument: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    argument
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| "provide --database-url, OPENTK_DATABASE_URL, or DATABASE_URL".into())
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

#[cfg(test)]
mod tests {
    use super::{categories_or_all, Cli, Command};
    use clap::Parser;

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
}
