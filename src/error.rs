use std::path::PathBuf;
use thiserror::Error;

/// Core library errors
#[derive(Error, Debug)]
pub enum SweeperError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error at path '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    ReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config file '{path}': {source}")]
    ParseError {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, SweeperError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = ConfigError::Invalid("threshold must be 0-100".into());
        assert!(err.to_string().contains("threshold"));
    }

    #[test]
    fn error_conversion() {
        let config_err = ConfigError::Invalid("test".into());
        let sweeper_err: SweeperError = config_err.into();
        assert!(matches!(sweeper_err, SweeperError::Config(_)));
    }
}
