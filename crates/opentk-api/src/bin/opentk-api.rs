use clap::Parser;
use opentk_api::{serve, ApiConfig, SearchBackendConfig};
use opentk_config::ConfigLoader;
use opentk_db::DatabaseConfig;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let loaded = match args.config {
        Some(path) => ConfigLoader::with_path(path).load()?,
        None => ConfigLoader::new().load()?,
    };
    let config = loaded.config;
    tracing_subscriber::fmt::init();
    tracing::info!(config = ?config.redacted(), "loaded effective config");

    serve(ApiConfig {
        bind_address: config.api.bind_address,
        database: DatabaseConfig {
            url: config.database.url,
            max_connections: config.database.max_connections,
        },
        search: SearchBackendConfig {
            url: config.search.url,
            api_key: config.search.api_key,
            index_name: config.search.index_name,
        },
    })
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn api_cli_accepts_only_config_path_for_application_settings() {
        let args = Args::try_parse_from(["opentk-api", "--config", "/etc/opentk/config.toml"])
            .expect("config path parses");
        assert_eq!(
            args.config.as_deref(),
            Some(std::path::Path::new("/etc/opentk/config.toml"))
        );

        for forbidden in ["--database-url", "--bind-address"] {
            let result = Args::try_parse_from(["opentk-api", forbidden, "value"]);
            assert!(result.is_err(), "{forbidden} must be rejected");
        }
    }
}
