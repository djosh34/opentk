//! Typed application configuration loaded from TOML files only.

use std::{
    env,
    error::Error as StdError,
    fs,
    net::SocketAddr,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use opentk_core::official_schema;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing_subscriber::{filter::ParseError, EnvFilter};

const LOCAL_CONFIG_PATH: &str = "./opentk.toml";
const SYSTEM_CONFIG_PATH: &str = "/etc/opentk/config.toml";
const LOG_FORMAT_ENV: &str = "OPENTK_LOG_FORMAT";
const RUST_LOG_ENV: &str = "RUST_LOG";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub database: DatabaseConfig,
    pub sync: SyncConfig,
    pub search: SearchConfig,
    pub api: ApiConfig,
    pub log: LogConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncConfig {
    pub base_url: Url,
    pub request_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub max_retries: u32,
    pub max_concurrent_requests: NonZeroUsize,
    pub poll_interval_secs: u64,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub index_name: String,
    pub batch_size: i64,
    pub max_payload_bytes: usize,
    pub retry_limit: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiConfig {
    pub bind_address: SocketAddr,
    pub cors_origins: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogConfig {
    pub format: LogFormat,
    pub level: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveLogConfig {
    pub format: LogFormat,
    pub level: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Pretty,
}

#[derive(Debug, Error)]
pub enum LogInitError {
    #[error("invalid OPENTK_LOG_FORMAT value {value:?}; expected json or pretty")]
    InvalidFormat { value: String },
    #[error("invalid tracing filter {value:?}")]
    InvalidFilter {
        value: String,
        #[source]
        source: ParseError,
    },
    #[error("failed to initialize tracing subscriber")]
    SubscriberInit(#[source] Box<dyn StdError + Send + Sync>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: Config,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RedactedConfig {
    pub database: RedactedDatabaseConfig,
    pub sync: RedactedSyncConfig,
    pub search: RedactedSearchConfig,
    pub api: RedactedApiConfig,
    pub log: RedactedLogConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RedactedDatabaseConfig {
    pub max_connections: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RedactedSyncConfig {
    pub base_url: String,
    pub request_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub max_retries: u32,
    pub max_concurrent_requests: usize,
    pub poll_interval_secs: u64,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RedactedSearchConfig {
    pub url: String,
    pub api_key: Option<&'static str>,
    pub index_name: String,
    pub batch_size: i64,
    pub max_payload_bytes: usize,
    pub retry_limit: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RedactedApiConfig {
    pub bind_address: String,
    pub cors_origins: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RedactedLogConfig {
    pub format: LogFormat,
    pub level: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigLoader {
    explicit_path: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file {path} does not exist")]
    MissingExplicitFile { path: PathBuf },
    #[error("no config file found; checked ./opentk.toml and /etc/opentk/config.toml")]
    NoConfigFile,
    #[error("failed to read config file {path}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    ParseFile {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to parse config: {0}")]
    ParseString(#[from] toml::de::Error),
    #[error("missing required config field {field}")]
    MissingRequired { field: &'static str },
    #[error("invalid config field {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
}

impl ConfigLoader {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            explicit_path: None,
        }
    }

    #[must_use]
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            explicit_path: Some(path.into()),
        }
    }

    /// Load configuration from the explicit path or well-known default paths.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when no file can be found, the selected file
    /// cannot be read, TOML parsing fails, or validation rejects the config.
    pub fn load(self) -> Result<LoadedConfig, ConfigError> {
        let path = self.selected_path()?;
        let source = fs::read_to_string(&path).map_err(|source| ConfigError::ReadFile {
            path: path.clone(),
            source,
        })?;
        let config = Config::from_toml_str(&source).map_err(|error| match error {
            ConfigError::ParseString(source) => ConfigError::ParseFile {
                path: path.clone(),
                source,
            },
            error => error,
        })?;
        Ok(LoadedConfig { path, config })
    }

    fn selected_path(&self) -> Result<PathBuf, ConfigError> {
        if let Some(path) = &self.explicit_path {
            if path.exists() {
                return Ok(path.clone());
            }
            return Err(ConfigError::MissingExplicitFile { path: path.clone() });
        }
        default_config_paths()
            .into_iter()
            .find(|path| path.exists())
            .ok_or(ConfigError::NoConfigFile)
    }
}

impl Config {
    /// Parse and validate config from a TOML string, applying compiled defaults
    /// only for optional fields.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when TOML parsing fails or validation rejects the
    /// config.
    pub fn from_toml_str(source: &str) -> Result<Self, ConfigError> {
        RawConfig::from_toml_str(source)?.try_into()
    }

    #[must_use]
    pub fn redacted(&self) -> RedactedConfig {
        RedactedConfig {
            database: RedactedDatabaseConfig {
                max_connections: self.database.max_connections,
            },
            sync: RedactedSyncConfig {
                base_url: self.sync.base_url.to_string(),
                request_timeout_secs: self.sync.request_timeout_secs,
                connect_timeout_secs: self.sync.connect_timeout_secs,
                max_retries: self.sync.max_retries,
                max_concurrent_requests: self.sync.max_concurrent_requests.get(),
                poll_interval_secs: self.sync.poll_interval_secs,
                categories: self.sync.categories.clone(),
            },
            search: RedactedSearchConfig {
                url: self.search.url.clone(),
                api_key: self.search.api_key.as_ref().map(|_| "***"),
                index_name: self.search.index_name.clone(),
                batch_size: self.search.batch_size,
                max_payload_bytes: self.search.max_payload_bytes,
                retry_limit: self.search.retry_limit,
            },
            api: RedactedApiConfig {
                bind_address: self.api.bind_address.to_string(),
                cors_origins: self.api.cors_origins.clone(),
            },
            log: RedactedLogConfig {
                format: self.log.format,
                level: self.log.level.clone(),
            },
        }
    }
}

/// Initialize process-wide tracing from config plus deployment overrides.
///
/// # Errors
///
/// Returns [`LogInitError`] when `OPENTK_LOG_FORMAT` is unsupported,
/// `RUST_LOG`/`log.level` is not a valid tracing filter, or a subscriber was
/// already installed.
pub fn init_tracing(config: &LogConfig) -> Result<(), LogInitError> {
    let effective = EffectiveLogConfig::from_env(config)?;
    let filter =
        EnvFilter::try_new(&effective.level).map_err(|source| LogInitError::InvalidFilter {
            value: effective.level.clone(),
            source,
        })?;

    match effective.format {
        LogFormat::Json => tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_env_filter(filter)
            .try_init()
            .map_err(LogInitError::SubscriberInit)?,
        LogFormat::Pretty => tracing_subscriber::fmt()
            .pretty()
            .with_ansi(false)
            .with_env_filter(filter)
            .try_init()
            .map_err(LogInitError::SubscriberInit)?,
    }
    Ok(())
}

impl EffectiveLogConfig {
    /// Resolve logging settings from explicit override values.
    ///
    /// # Errors
    ///
    /// Returns [`LogInitError`] when `format_override` is unsupported.
    pub fn from_overrides(
        config: &LogConfig,
        format_override: Option<&str>,
        level_override: Option<&str>,
    ) -> Result<Self, LogInitError> {
        let format = format_override.map_or(Ok(config.format), parse_log_format)?;
        let level = level_override.map_or_else(|| config.level.clone(), ToOwned::to_owned);
        Ok(Self { format, level })
    }

    /// Resolve logging settings from `[log]`, `OPENTK_LOG_FORMAT`, and
    /// `RUST_LOG`.
    ///
    /// # Errors
    ///
    /// Returns [`LogInitError`] when `OPENTK_LOG_FORMAT` is unsupported.
    pub fn from_env(config: &LogConfig) -> Result<Self, LogInitError> {
        let format = env::var(LOG_FORMAT_ENV).ok();
        let level = env::var(RUST_LOG_ENV).ok();
        Self::from_overrides(config, format.as_deref(), level.as_deref())
    }
}

fn parse_log_format(value: &str) -> Result<LogFormat, LogInitError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "json" => Ok(LogFormat::Json),
        "pretty" => Ok(LogFormat::Pretty),
        _ => Err(LogInitError::InvalidFormat {
            value: value.to_owned(),
        }),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    database: Option<RawDatabaseConfig>,
    sync: Option<RawSyncConfig>,
    search: Option<RawSearchConfig>,
    api: Option<RawApiConfig>,
    log: Option<RawLogConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDatabaseConfig {
    url: Option<String>,
    max_connections: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSyncConfig {
    base_url: Option<String>,
    request_timeout_secs: Option<u64>,
    connect_timeout_secs: Option<u64>,
    max_retries: Option<u32>,
    max_concurrent_requests: Option<usize>,
    poll_interval_secs: Option<u64>,
    categories: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSearchConfig {
    url: Option<String>,
    api_key: Option<String>,
    index_name: Option<String>,
    batch_size: Option<i64>,
    max_payload_bytes: Option<usize>,
    retry_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApiConfig {
    bind_address: Option<String>,
    cors_origins: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLogConfig {
    format: Option<LogFormat>,
    level: Option<String>,
}

impl RawConfig {
    fn from_toml_str(source: &str) -> Result<Self, ConfigError> {
        toml::from_str(source).map_err(ConfigError::ParseString)
    }
}

impl TryFrom<RawConfig> for Config {
    type Error = ConfigError;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        let database = raw.database.unwrap_or_default_config().try_into()?;
        Ok(Self {
            database,
            sync: raw.sync.unwrap_or_default_config().try_into()?,
            search: raw.search.unwrap_or_default_config().try_into()?,
            api: raw.api.unwrap_or_default_config().try_into()?,
            log: raw.log.unwrap_or_default_config().try_into()?,
        })
    }
}

impl TryFrom<RawDatabaseConfig> for DatabaseConfig {
    type Error = ConfigError;

    fn try_from(raw: RawDatabaseConfig) -> Result<Self, Self::Error> {
        let url = required_non_empty(raw.url, "database.url")?;
        let max_connections = raw.max_connections.unwrap_or(5);
        ensure_positive_u32(max_connections, "database.max_connections")?;
        Ok(Self {
            url,
            max_connections,
        })
    }
}

impl TryFrom<RawSyncConfig> for SyncConfig {
    type Error = ConfigError;

    fn try_from(raw: RawSyncConfig) -> Result<Self, Self::Error> {
        let base_url = raw
            .base_url
            .unwrap_or_else(|| "https://gegevensmagazijn.tweedekamer.nl".to_owned());
        let base_url = parse_url(&base_url, "sync.base_url")?;
        let max_concurrent_requests = non_zero_usize(
            raw.max_concurrent_requests.unwrap_or(4),
            "sync.max_concurrent_requests",
        )?;
        Ok(Self {
            base_url,
            request_timeout_secs: raw.request_timeout_secs.unwrap_or(30),
            connect_timeout_secs: raw.connect_timeout_secs.unwrap_or(10),
            max_retries: raw.max_retries.unwrap_or(3),
            max_concurrent_requests,
            poll_interval_secs: raw.poll_interval_secs.unwrap_or(30),
            categories: raw.categories.unwrap_or_else(official_categories),
        })
    }
}

impl TryFrom<RawSearchConfig> for SearchConfig {
    type Error = ConfigError;

    fn try_from(raw: RawSearchConfig) -> Result<Self, Self::Error> {
        let batch_size = raw.batch_size.unwrap_or(100);
        if batch_size <= 0 {
            return Err(invalid("search.batch_size", "must be greater than zero"));
        }
        let retry_limit = raw.retry_limit.unwrap_or(3);
        if retry_limit <= 0 {
            return Err(invalid("search.retry_limit", "must be greater than zero"));
        }
        let max_payload_bytes = raw.max_payload_bytes.unwrap_or(80_000_000);
        if max_payload_bytes == 0 {
            return Err(invalid(
                "search.max_payload_bytes",
                "must be greater than zero",
            ));
        }
        Ok(Self {
            url: non_empty_or_default(raw.url, "http://meilisearch:7700", "search.url")?,
            api_key: raw.api_key,
            index_name: non_empty_or_default(
                raw.index_name,
                "opentk_entities",
                "search.index_name",
            )?,
            batch_size,
            max_payload_bytes,
            retry_limit,
        })
    }
}

impl TryFrom<RawApiConfig> for ApiConfig {
    type Error = ConfigError;

    fn try_from(raw: RawApiConfig) -> Result<Self, Self::Error> {
        let bind_address = raw
            .bind_address
            .unwrap_or_else(|| "0.0.0.0:3000".to_owned())
            .parse::<SocketAddr>()
            .map_err(|source| ConfigError::InvalidField {
                field: "api.bind_address",
                message: source.to_string(),
            })?;
        Ok(Self {
            bind_address,
            cors_origins: raw.cors_origins.unwrap_or_default(),
        })
    }
}

impl TryFrom<RawLogConfig> for LogConfig {
    type Error = ConfigError;

    fn try_from(raw: RawLogConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            format: raw.format.unwrap_or(LogFormat::Pretty),
            level: non_empty_or_default(raw.level, "info", "log.level")?,
        })
    }
}

trait OptionalConfig<T> {
    fn unwrap_or_default_config(self) -> T;
}

impl OptionalConfig<RawDatabaseConfig> for Option<RawDatabaseConfig> {
    fn unwrap_or_default_config(self) -> RawDatabaseConfig {
        self.unwrap_or(RawDatabaseConfig {
            url: None,
            max_connections: None,
        })
    }
}

impl OptionalConfig<RawSyncConfig> for Option<RawSyncConfig> {
    fn unwrap_or_default_config(self) -> RawSyncConfig {
        self.unwrap_or(RawSyncConfig {
            base_url: None,
            request_timeout_secs: None,
            connect_timeout_secs: None,
            max_retries: None,
            max_concurrent_requests: None,
            poll_interval_secs: None,
            categories: None,
        })
    }
}

impl OptionalConfig<RawSearchConfig> for Option<RawSearchConfig> {
    fn unwrap_or_default_config(self) -> RawSearchConfig {
        self.unwrap_or(RawSearchConfig {
            url: None,
            api_key: None,
            index_name: None,
            batch_size: None,
            max_payload_bytes: None,
            retry_limit: None,
        })
    }
}

impl OptionalConfig<RawApiConfig> for Option<RawApiConfig> {
    fn unwrap_or_default_config(self) -> RawApiConfig {
        self.unwrap_or(RawApiConfig {
            bind_address: None,
            cors_origins: None,
        })
    }
}

impl OptionalConfig<RawLogConfig> for Option<RawLogConfig> {
    fn unwrap_or_default_config(self) -> RawLogConfig {
        self.unwrap_or(RawLogConfig {
            format: None,
            level: None,
        })
    }
}

fn required_non_empty(value: Option<String>, field: &'static str) -> Result<String, ConfigError> {
    let value = value.ok_or(ConfigError::MissingRequired { field })?;
    if value.trim().is_empty() {
        Err(invalid(field, "must not be empty"))
    } else {
        Ok(value)
    }
}

fn non_empty_or_default(
    value: Option<String>,
    default: &'static str,
    field: &'static str,
) -> Result<String, ConfigError> {
    let value = value.unwrap_or_else(|| default.to_owned());
    if value.trim().is_empty() {
        Err(invalid(field, "must not be empty"))
    } else {
        Ok(value)
    }
}

fn parse_url(value: &str, field: &'static str) -> Result<Url, ConfigError> {
    Url::parse(value).map_err(|source| ConfigError::InvalidField {
        field,
        message: source.to_string(),
    })
}

fn ensure_positive_u32(value: u32, field: &'static str) -> Result<(), ConfigError> {
    if value == 0 {
        Err(invalid(field, "must be greater than zero"))
    } else {
        Ok(())
    }
}

fn non_zero_usize(value: usize, field: &'static str) -> Result<NonZeroUsize, ConfigError> {
    NonZeroUsize::new(value).ok_or_else(|| invalid(field, "must be greater than zero"))
}

fn invalid(field: &'static str, message: impl Into<String>) -> ConfigError {
    ConfigError::InvalidField {
        field,
        message: message.into(),
    }
}

fn official_categories() -> Vec<String> {
    official_schema::entity_types()
        .iter()
        .map(|entity| entity.category.to_owned())
        .collect()
}

fn default_config_paths() -> [PathBuf; 2] {
    [
        Path::new(LOCAL_CONFIG_PATH).into(),
        Path::new(SYSTEM_CONFIG_PATH).into(),
    ]
}
