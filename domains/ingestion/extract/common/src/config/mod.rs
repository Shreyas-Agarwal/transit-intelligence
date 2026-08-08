//! Environment-based configuration loading.
//!
//! Individual crates define their own config structs (e.g. `ckan::config::CkanConfig`)
//! built from these primitives, rather than this module trying to know about every
//! crate's specific settings.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingVar(String),
    #[error("environment variable {name} has an invalid value {value:?}: {reason}")]
    InvalidVar {
        name: String,
        value: String,
        reason: String,
    },
}

/// Loads a `.env` file from the current directory (or an ancestor) if present.
///
/// Safe to call multiple times and safe to call when no `.env` file exists — this
/// project's `.env` is gitignored and optional in CI/production, where real
/// environment variables are expected to be set directly.
pub fn load_dotenv() {
    // Errors (missing file) are intentionally ignored: absence of a `.env` is the
    // normal case outside local development.
    let _ = dotenvy::dotenv();
}

/// Reads a required environment variable, producing a descriptive error if unset.
pub fn require_env(name: &str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError::MissingVar(name.to_string()))
}

/// Reads an optional environment variable, falling back to `default` if unset.
pub fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Reads an optional environment variable as a filesystem path, falling back to
/// `default` if unset.
pub fn env_path_or(name: &str, default: &str) -> PathBuf {
    std::env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

/// Reads an optional environment variable and parses it, falling back to `default`
/// if unset. A value that's set but fails to parse is a hard config error rather
/// than a silent fallback, since that almost always indicates a typo.
pub fn env_parsed_or<T>(name: &str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(value) => value.parse::<T>().map_err(|e| ConfigError::InvalidVar {
            name: name.to_string(),
            value,
            reason: e.to_string(),
        }),
        Err(_) => Ok(default),
    }
}
