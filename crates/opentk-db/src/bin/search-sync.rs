use std::path::PathBuf;

use clap::{Parser, Subcommand};
use opentk_config::{Config, ConfigLoader};
use opentk_db::connect;
use opentk_db::search_sync::{
    full_reindex, incremental_index, list_failures, SearchSyncConfig, SearchSyncReport,
};
use opentk_db::DatabaseConfig;
use opentk_search::MeilisearchClient;

#[derive(Debug, Parser)]
#[command(name = "search-sync")]
#[command(about = "Build or update the OpenTK Meilisearch index from PostgreSQL")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    FullReindex,
    Incremental,
    Failures,
    RetryFailures,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let loaded = match cli.config {
        Some(path) => ConfigLoader::with_path(path).load()?,
        None => ConfigLoader::new().load()?,
    };
    match cli.command {
        Command::FullReindex => {
            let report = run_sync(&loaded.config, true).await?;
            print_report(&report);
        }
        Command::Incremental | Command::RetryFailures => {
            let report = run_sync(&loaded.config, false).await?;
            print_report(&report);
        }
        Command::Failures => {
            let pool = connect(&database_config(&loaded.config)).await?;
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
    config: &Config,
    full: bool,
) -> Result<SearchSyncReport, Box<dyn std::error::Error>> {
    let pool = connect(&database_config(config)).await?;
    let sync_config = search_sync_config(config);
    let client = MeilisearchClient::new(
        config.search.url.clone(),
        config.search.api_key.clone(),
        sync_config.index_name.clone(),
    );
    if full {
        Ok(full_reindex(&pool, &client, &sync_config).await?)
    } else {
        Ok(incremental_index(&pool, &client, &sync_config).await?)
    }
}

fn search_sync_config(config: &Config) -> SearchSyncConfig {
    SearchSyncConfig {
        index_name: config.search.index_name.clone(),
        categories: config.sync.categories.clone(),
        batch_size: config.search.batch_size,
        retry_limit: config.search.retry_limit,
    }
}

fn database_config(config: &Config) -> DatabaseConfig {
    DatabaseConfig {
        url: config.database.url.clone(),
        max_connections: config.database.max_connections,
    }
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

#[cfg(test)]
mod tests {
    use super::{search_sync_config, Cli, Command};
    use clap::Parser;
    use opentk_config::Config;

    #[test]
    fn search_sync_cli_accepts_only_config_path_for_application_settings() {
        let cli = Cli::try_parse_from([
            "search-sync",
            "--config",
            "/etc/opentk/config.toml",
            "full-reindex",
        ])
        .expect("config path parses");
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/etc/opentk/config.toml"))
        );
        let Command::FullReindex = cli.command else {
            panic!("expected full reindex");
        };
    }

    #[test]
    fn old_application_setting_flags_are_rejected() {
        for args in [
            [
                "search-sync",
                "full-reindex",
                "--database-url",
                "postgres://example.test/db",
            ],
            [
                "search-sync",
                "full-reindex",
                "--meilisearch-url",
                "http://search:7700",
            ],
            [
                "search-sync",
                "full-reindex",
                "--meilisearch-api-key",
                "secret",
            ],
            ["search-sync", "full-reindex", "--index-name", "custom"],
            ["search-sync", "full-reindex", "--category", "Document"],
            ["search-sync", "full-reindex", "--batch-size", "10"],
            ["search-sync", "full-reindex", "--retry-limit", "2"],
        ] {
            assert!(Cli::try_parse_from(args).is_err(), "{args:?} must fail");
        }
    }

    #[test]
    fn search_sync_config_uses_official_default_categories() {
        let config = Config::from_toml_str(
            r#"
            [database]
            url = "postgres://postgres@example.test/opentk"
            "#,
        )
        .expect("config loads");

        let sync_config = search_sync_config(&config);

        assert!(sync_config.categories.contains(&"Document".to_owned()));
        assert!(sync_config.categories.contains(&"Zaak".to_owned()));
        assert!(sync_config.categories.len() > 2);
        assert_eq!(sync_config.index_name, "opentk_entities");
        assert_eq!(sync_config.batch_size, 100);
        assert_eq!(sync_config.retry_limit, 3);
    }

    #[test]
    fn search_sync_config_uses_user_provided_categories_and_search_limits() {
        let config = Config::from_toml_str(
            r#"
            [database]
            url = "postgres://postgres@example.test/opentk"

            [sync]
            categories = ["Document"]

            [search]
            index_name = "custom"
            batch_size = 25
            retry_limit = 4
            "#,
        )
        .expect("config loads");

        let sync_config = search_sync_config(&config);

        assert_eq!(sync_config.categories, ["Document"]);
        assert_eq!(sync_config.index_name, "custom");
        assert_eq!(sync_config.batch_size, 25);
        assert_eq!(sync_config.retry_limit, 4);
    }
}
