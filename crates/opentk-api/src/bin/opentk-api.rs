use clap::Parser;
use opentk_api::{serve, ApiConfig};
use std::{env, net::SocketAddr};
use thiserror::Error;

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        env = "OPENTK_API_BIND_ADDRESS",
        default_value = "127.0.0.1:3000"
    )]
    bind_address: SocketAddr,
    #[arg(long, env = "OPENTK_DATABASE_URL")]
    database_url: Option<String>,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("set --database-url, OPENTK_DATABASE_URL, or DATABASE_URL")]
    MissingDatabaseUrl,
    #[error("DATABASE_URL is not valid Unicode")]
    InvalidDatabaseUrl(#[source] env::VarError),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let database_url = match args.database_url {
        Some(database_url) => database_url,
        None => match env::var("DATABASE_URL") {
            Ok(database_url) => database_url,
            Err(env::VarError::NotPresent) => return Err(CliError::MissingDatabaseUrl.into()),
            Err(error @ env::VarError::NotUnicode(_)) => {
                return Err(CliError::InvalidDatabaseUrl(error).into());
            }
        },
    };

    serve(ApiConfig {
        bind_address: args.bind_address,
        database_url,
    })
    .await?;

    Ok(())
}
