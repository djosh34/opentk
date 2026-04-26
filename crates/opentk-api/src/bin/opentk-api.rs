use clap::Parser;
use opentk_api::{serve, ApiConfig, SearchBackendConfig};
use opentk_config::{ConfigLoader, LogFormat};
use opentk_db::DatabaseConfig;
use std::path::PathBuf;
use tracing_subscriber::{filter::ParseError, EnvFilter};

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
    init_logging(loaded.config.log.format, &loaded.config.log.level)?;

    serve(ApiConfig {
        bind_address: loaded.config.api.bind_address,
        database: DatabaseConfig {
            url: loaded.config.database.url,
            max_connections: loaded.config.database.max_connections,
        },
        search: SearchBackendConfig {
            url: loaded.config.search.url,
            api_key: loaded.config.search.api_key,
            index_name: loaded.config.search.index_name,
        },
    })
    .await?;

    Ok(())
}

fn init_logging(format: LogFormat, level: &str) -> Result<(), ParseError> {
    let filter = EnvFilter::try_new(level)?;
    match format {
        LogFormat::Json => tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init(),
        LogFormat::Pretty => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
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
