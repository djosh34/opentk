use clap::Parser;
use opentk_api::{serve, ApiConfig, SearchBackendConfig};
use opentk_config::{init_tracing, Config, ConfigLoader};
use opentk_db::{
    startup_validation::{validate_api_dependencies, SearchRequirement},
    DatabaseConfig,
};
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    validate_config: bool,
    #[arg(long, requires = "validate_config")]
    require_search: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let loaded = match args.config {
        Some(path) => ConfigLoader::with_path(path).load()?,
        None => ConfigLoader::new().load()?,
    };
    let config = loaded.config;
    init_tracing(&config.log)?;
    tracing::info!(config = ?config.redacted(), "loaded effective config");

    if args.validate_config {
        let requirement = if args.require_search {
            SearchRequirement::Required
        } else {
            SearchRequirement::Optional
        };
        let report = validate_api_dependencies(&config, requirement).await?;
        print!("{report}");
        return Ok(());
    }

    serve(api_config(config)).await?;

    Ok(())
}

fn api_config(config: Config) -> ApiConfig {
    ApiConfig {
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
        max_public_query_limit: config.api.max_public_query_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn api_cli_accepts_config_path_and_validation_flags() {
        let args = Args::try_parse_from(["opentk-api", "--config", "/etc/opentk/config.toml"])
            .expect("config path parses");
        assert_eq!(
            args.config.as_deref(),
            Some(std::path::Path::new("/etc/opentk/config.toml"))
        );
        assert!(!args.validate_config);
        assert!(!args.require_search);

        let args = Args::try_parse_from(["opentk-api", "--validate-config", "--require-search"])
            .expect("validation flags parse");
        assert!(args.validate_config);
        assert!(args.require_search);

        assert!(Args::try_parse_from(["opentk-api", "--require-search"]).is_err());

        for forbidden in ["--database-url", "--bind-address"] {
            let result = Args::try_parse_from(["opentk-api", forbidden, "value"]);
            assert!(result.is_err(), "{forbidden} must be rejected");
        }
    }
}
